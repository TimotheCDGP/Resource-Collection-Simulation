//! Tests : quelques vérifications unitaires de la navigation et de la
//! connaissance, plus un test d'intégration qui fait réellement tourner la
//! simulation concurrente quelques instants et vérifie ses invariants.

use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::knowledge::Knowledge;
use crate::map::Map;
use crate::pathfinding::{bfs_first_steps, random_step};
use crate::robot::{run_collector, run_scout};
use crate::world::{Cell, News, Report, ResourceKind, RobotKind, World};

#[test]
fn bfs_atteint_la_grille_ouverte_et_contourne_les_murs() {
    // Grille 3x3 entièrement ouverte : toutes les cases sont atteignables.
    let steps = bfs_first_steps((0, 0), 3, 3, |_| true);
    assert_eq!(steps.len(), 9);
    assert!(steps.contains_key(&(2, 2)));

    // On mure les deux seuls accès de (2,2) : elle devient inatteignable.
    let steps = bfs_first_steps((0, 0), 3, 3, |p| p != (1, 2) && p != (2, 1));
    assert!(!steps.contains_key(&(2, 2)));
}

#[test]
fn random_step_reste_sur_place_si_tout_est_bloque() {
    let mut rng = rand::thread_rng();
    // Aucune voisine libre -> on reste sur place.
    assert_eq!(random_step((1, 1), 3, 3, |_| false, &mut rng), (1, 1));
    // Une seule voisine libre -> on y va.
    assert_eq!(random_step((1, 1), 3, 3, |c| c == (1, 0), &mut rng), (1, 0));
}

#[test]
fn la_connaissance_part_inconnue_puis_se_met_a_jour() {
    let mut k = Knowledge::new(4, 4);
    assert!(k.passable((2, 2))); // inconnu = supposé franchissable
    k.note_cell((2, 2), Cell::Obstacle);
    assert!(!k.passable((2, 2)));

    k.note_resource((1, 1), ResourceKind::Energy);
    assert!(k.resources.contains_key(&(1, 1)));
    k.forget_resource(&(1, 1));
    assert!(!k.resources.contains_key(&(1, 1)));
}

/// Lance la vraie simulation pendant ~1,5 s et vérifie qu'elle progresse :
/// la connaissance circule, les robots explorent, et en régime établi deux
/// robots ne partagent jamais une case.
#[test]
fn la_simulation_tourne_partage_et_evite_les_collisions() {
    let (map, resources) = Map::new(80, 25);
    let map = Arc::new(map);
    let kinds = vec![
        RobotKind::Scout,
        RobotKind::Scout,
        RobotKind::Scout,
        RobotKind::Collector,
        RobotKind::Collector,
    ];
    let world = Arc::new(Mutex::new(World {
        positions: vec![map.base; kinds.len()],
        resources,
    }));

    let (report_tx, report_rx) = mpsc::channel::<Report>();
    let mut news_senders = Vec::new();
    for (id, &kind) in kinds.iter().enumerate() {
        let (news_tx, news_rx) = mpsc::channel::<News>();
        news_senders.push(news_tx);
        let (m, w, r) = (Arc::clone(&map), Arc::clone(&world), report_tx.clone());
        thread::spawn(move || match kind {
            RobotKind::Scout => run_scout(id, m, w, r, news_rx),
            RobotKind::Collector => run_collector(id, m, w, r, news_rx),
        });
    }
    drop(report_tx);

    // On joue le rôle de la base : on relève, on agrège, on rediffuse.
    let mut board = Knowledge::with_base_seen(&map);
    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut last_positions = Vec::new();

    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        let (mut cells, mut res, mut gone) = (Vec::new(), Vec::new(), Vec::new());
        while let Ok(report) = report_rx.try_recv() {
            if let Report::Seen {
                cells: c,
                resources: r,
                gone_resources: g,
            } = report
            {
                for (p, cell) in c {
                    board.note_cell(p, cell);
                    cells.push((p, cell));
                }
                for (p, kind) in r {
                    board.note_resource(p, kind);
                    res.push((p, kind));
                }
                for p in g {
                    board.forget_resource(&p);
                    gone.push(p);
                }
            }
        }
        if !cells.is_empty() || !res.is_empty() || !gone.is_empty() {
            let news = News {
                cells,
                resources: res,
                gone_resources: gone,
            };
            for sender in &news_senders {
                let _ = sender.send(news.clone());
            }
        }
        last_positions = world.lock().unwrap().positions.clone();
        visited.extend(last_positions.iter().copied());
        thread::sleep(Duration::from_millis(10));
    }

    // 1) La connaissance a circulé : la base sait plus que le carré de départ.
    let known = board
        .cells
        .iter()
        .flatten()
        .filter(|c| **c != Cell::Unknown)
        .count();
    assert!(
        known > 9,
        "la base devrait avoir agrégé des découvertes (connu = {known})"
    );

    // 2) Les robots ont exploré : plus de cases visitées que de robots.
    assert!(
        visited.len() > kinds.len(),
        "les robots devraient se déplacer"
    );

    // 3) En régime établi, pas deux robots sur la même case.
    let unique: HashSet<_> = last_positions.iter().collect();
    assert_eq!(
        unique.len(),
        last_positions.len(),
        "collision détectée : {last_positions:?}"
    );
}
