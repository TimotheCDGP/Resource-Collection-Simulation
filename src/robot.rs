//! Comportement d'un robot (un thread chacun).
//!
//! Chaque robot raisonne sur SA propre carte mentale ([`Knowledge`]) et ne
//! touche au monde partagé que brièvement, pour deux choses qui exigent une
//! cohérence physique : lire/écrire sa position (anti-collision) et ramasser une
//! ressource (épuisement du stock). Tout le partage de connaissance passe par
//! les canaux : le robot envoie ce qu'il découvre à la base, et reçoit en retour
//! les découvertes des autres.

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rand::rngs::ThreadRng;

use crate::knowledge::{around, Knowledge};
use crate::map::Map;
use crate::pathfinding::{bfs_first_steps, random_step};
use crate::world::{News, Pos, Report, ResourceKind, SeenCells, SeenResources, World};

/// Cadence d'un tick de robot.
const TICK: Duration = Duration::from_millis(150);
/// Rayon de perception d'un éclaireur (le collecteur, lui, voit à 1).
const SCOUT_VISION: i32 = 2;

/// Éclaireur : marche au hasard, perçoit large et signale ses trouvailles. Il ne
/// ramasse jamais de ressource.
pub(crate) fn run_scout(
    id: usize,
    map: Arc<Map>,
    world: Arc<Mutex<World>>,
    to_base: Sender<Report>,
    from_base: Receiver<News>,
) {
    let mut rng = rand::thread_rng();
    let mut mind = Knowledge::with_base_seen(&map);

    loop {
        absorb_news(&mut mind, &from_base);

        let (new_cells, new_res) = {
            let mut w = world.lock().unwrap();
            let pos = w.positions[id];
            let occupied = others(&w.positions, id);

            // Perçoit l'environnement et n'en retient que la nouveauté.
            let (cells, res) = perceive(pos, &map, &w, SCOUT_VISION, &mut mind);

            // Avance vers une case réellement libre et inoccupée.
            let next = random_step(pos, map.width, map.height, |p| {
                map.passable(p) && !occupied.contains(&p)
            }, &mut rng);
            w.positions[id] = next;
            (cells, res)
        };

        report(&to_base, new_cells, new_res, None);
        thread::sleep(TICK);
    }
}

/// Collecteur : va chercher le gisement connu le plus proche, le rapporte à la
/// base, recommence. Il planifie sur sa propre carte mentale.
pub(crate) fn run_collector(
    id: usize,
    map: Arc<Map>,
    world: Arc<Mutex<World>>,
    to_base: Sender<Report>,
    from_base: Receiver<News>,
) {
    let mut rng = rand::thread_rng();
    let mut mind = Knowledge::with_base_seen(&map);
    let base = map.base;
    let mut carrying: Option<ResourceKind> = None;

    loop {
        absorb_news(&mut mind, &from_base);

        // 1. Perception + éventuel dépôt/ramassage, sous un court verrou.
        let (pos, new_cells, new_res, delivered, acted) = {
            let mut w = world.lock().unwrap();
            let pos = w.positions[id];
            let (cells, res) = perceive(pos, &map, &w, 1, &mut mind);

            let mut delivered = None;
            let acted;
            if carrying.is_some() && pos == base {
                // Décharge à la base.
                delivered = carrying.take();
                acted = true;
            } else if carrying.is_none() {
                match w.resources.get(&pos).copied() {
                    Some((kind, qty)) if qty > 0 => {
                        // Ramasse une unité ; retire le gisement s'il est vidé.
                        if qty == 1 {
                            w.resources.remove(&pos);
                        } else {
                            w.resources.insert(pos, (kind, qty - 1));
                        }
                        carrying = Some(kind);
                        acted = true;
                    }
                    _ => {
                        // Plus rien ici : on oublie ce gisement pour ne pas y revenir.
                        mind.forget_resource(&pos);
                        acted = false;
                    }
                }
            } else {
                acted = false;
            }
            (pos, cells, res, delivered, acted)
        };

        report(&to_base, new_cells, new_res, delivered);

        // 2. Si on n'a ni déposé ni ramassé, on se déplace.
        if !acted {
            let next = plan_step(pos, base, carrying.is_some(), &mind, &map, &mut rng);
            let mut w = world.lock().unwrap();
            let occupied = others(&w.positions, id);
            let go = if map.passable(next) && !occupied.contains(&next) {
                next
            } else {
                // Case visée murée ou occupée : petit pas libre pour ne pas coincer.
                random_step(pos, map.width, map.height, |p| {
                    map.passable(p) && !occupied.contains(&p)
                }, &mut rng)
            };
            w.positions[id] = go;
        }

        thread::sleep(TICK);
    }
}

/// Décide la prochaine case d'un collecteur en planifiant sur SA carte mentale.
fn plan_step(
    pos: Pos,
    base: Pos,
    carrying: bool,
    mind: &Knowledge,
    map: &Map,
    rng: &mut ThreadRng,
) -> Pos {
    // BFS sur la carte connue (l'inconnu est supposé franchissable).
    let steps = bfs_first_steps(pos, map.width, map.height, |p| mind.passable(p));

    let target = if carrying {
        // On rentre à la base si un chemin connu y mène.
        steps.contains_key(&base).then_some(base)
    } else {
        // Sinon, le gisement connu atteignable le plus proche.
        mind.resources
            .keys()
            .copied()
            .filter(|p| steps.contains_key(p))
            .min_by_key(|p| manhattan(*p, pos))
    };

    match target {
        Some(t) if t != pos => steps.get(&t).copied().unwrap_or(pos),
        // Rien à viser : on explore au hasard en évitant les obstacles connus.
        _ => random_step(pos, map.width, map.height, |p| mind.passable(p), rng),
    }
}

/// Lit la connaissance fraîche rediffusée par la base et l'intègre.
fn absorb_news(mind: &mut Knowledge, from_base: &Receiver<News>) {
    while let Ok(news) = from_base.try_recv() {
        for (p, cell) in news.cells {
            mind.note_cell(p, cell);
        }
        for (p, kind) in news.resources {
            mind.note_resource(p, kind);
        }
    }
}

/// Perçoit le terrain et les ressources autour de `pos`, met à jour la carte
/// mentale, et renvoie UNIQUEMENT les nouveautés (à transmettre à la base).
fn perceive(
    pos: Pos,
    map: &Map,
    world: &World,
    radius: i32,
    mind: &mut Knowledge,
) -> (SeenCells, SeenResources) {
    let mut new_cells = Vec::new();
    let mut new_res = Vec::new();

    for (p, cell) in around(pos, map, radius) {
        if mind.cells[p.1][p.0] != cell {
            mind.note_cell(p, cell);
            new_cells.push((p, cell));
        }
        if let Some(&(kind, _)) = world.resources.get(&p) {
            if !mind.resources.contains_key(&p) {
                mind.note_resource(p, kind);
                new_res.push((p, kind));
            }
        }
    }
    (new_cells, new_res)
}

/// Envoie à la base ce qui a été vu (si nouveau) et l'éventuel dépôt.
fn report(
    to_base: &Sender<Report>,
    cells: SeenCells,
    resources: SeenResources,
    delivered: Option<ResourceKind>,
) {
    if !cells.is_empty() || !resources.is_empty() {
        let _ = to_base.send(Report::Seen { cells, resources });
    }
    if let Some(kind) = delivered {
        let _ = to_base.send(Report::Delivered(kind));
    }
}

/// Positions des AUTRES robots (pour l'anti-collision).
fn others(positions: &[Pos], id: usize) -> HashSet<Pos> {
    positions
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != id)
        .map(|(_, &p)| p)
        .collect()
}

fn manhattan(a: Pos, b: Pos) -> u32 {
    let dx = a.0 as i32 - b.0 as i32;
    let dy = a.1 as i32 - b.1 as i32;
    (dx.abs() + dy.abs()) as u32
}
