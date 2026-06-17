//! Orchestration et rôle de la BASE.
//!
//! La base génère le monde, lance un thread par robot, puis joue le rôle de
//! centre de communication : elle relève les rapports des robots (canal unique
//! robots -> base), agrège la connaissance commune, et la rediffuse à chacun
//! (un canal base -> robot par robot). Elle dessine aussi l'affichage et gère
//! le clavier. Le tout sans bloquer les robots : les échanges passent par des
//! canaux, le `Mutex` du monde n'est pris que pour de courtes lectures.

use std::collections::HashMap;
use std::io;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event};
use ratatui::Terminal;

use crate::knowledge::Knowledge;
use crate::map::Map;
use crate::robot::{run_collector, run_scout};
use crate::ui::ui;
use crate::world::{News, Pos, Report, ResourceKind, RobotKind, World};

pub(crate) fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let (map, resources) = Map::new(80, 25);
    let map = Arc::new(map);

    // 3 éclaireurs puis 2 collecteurs, tous au départ sur la base.
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

    // Canaux : un seul robots -> base, et un base -> robot par robot.
    let (report_tx, report_rx) = mpsc::channel::<Report>();
    let mut news_senders: Vec<mpsc::Sender<News>> = Vec::new();

    for (id, &kind) in kinds.iter().enumerate() {
        let (news_tx, news_rx) = mpsc::channel::<News>();
        news_senders.push(news_tx);

        let map_c = Arc::clone(&map);
        let world_c = Arc::clone(&world);
        let report_c = report_tx.clone();
        thread::spawn(move || match kind {
            RobotKind::Scout => run_scout(id, map_c, world_c, report_c, news_rx),
            RobotKind::Collector => run_collector(id, map_c, world_c, report_c, news_rx),
        });
    }
    drop(report_tx); // la base n'émet pas de rapport

    // Connaissance agrégée par la base (sert au rendu du brouillard) + compteurs.
    let mut board = Knowledge::with_base_seen(&map);
    let mut totals = (0u32, 0u32);

    loop {
        // 1. Relève les rapports, agrège, et rediffuse les nouveautés à tous.
        let mut fresh_cells = Vec::new();
        let mut fresh_res = Vec::new();
        let mut gone_res = Vec::new();
        while let Ok(report) = report_rx.try_recv() {
            match report {
                Report::Seen {
                    cells,
                    resources,
                    gone_resources,
                } => {
                    for (p, cell) in cells {
                        board.note_cell(p, cell);
                        fresh_cells.push((p, cell));
                    }
                    for (p, kind) in resources {
                        board.note_resource(p, kind);
                        fresh_res.push((p, kind));
                    }
                    for p in gone_resources {
                        board.forget_resource(&p);
                        gone_res.push(p);
                    }
                }
                Report::Delivered(kind) => match kind {
                    ResourceKind::Energy => totals.0 += 1,
                    ResourceKind::Crystal => totals.1 += 1,
                },
            }
        }
        if !fresh_cells.is_empty() || !fresh_res.is_empty() || !gone_res.is_empty() {
            let news = News {
                cells: fresh_cells,
                resources: fresh_res,
                gone_resources: gone_res,
            };
            for sender in &news_senders {
                let _ = sender.send(news.clone());
            }
        }

        // 2. Photo pour le rendu : positions réelles + gisements encore présents.
        let (positions, visible_res) = {
            let w = world.lock().unwrap();
            let visible: HashMap<Pos, ResourceKind> = board
                .resources
                .iter()
                .filter(|(p, _)| w.resources.contains_key(p))
                .map(|(&p, &k)| (p, k))
                .collect();
            (w.positions.clone(), visible)
        };

        terminal.draw(|f| {
            ui(
                f,
                &map,
                &board.cells,
                &visible_res,
                &positions,
                &kinds,
                totals,
            )
        })?;

        // 3. Toute pression de touche quitte (exigé par le sujet).
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    return Ok(());
                }
            }
        }
    }
}
