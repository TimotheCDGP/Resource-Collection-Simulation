//! Orchestration : génération du monde, spawn d'un thread par robot, drain
//! du canal d'événements, snapshot pour le rendu, gestion du clavier.

use std::collections::HashMap;
use std::io;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use ratatui::Terminal;

use crate::map::Map;
use crate::robot::{run_collector, run_scout};
use crate::ui::ui;
use crate::world::{ResourceKind, Robot, RobotEvent, RobotKind, WorldState};

pub(crate) fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
) -> io::Result<()> {
    let (map, initial_resources) = Map::new(80, 25);
    let map = Arc::new(map);

    let mut robots = Vec::new();
    for _ in 0..3 {
        robots.push(Robot {
            pos: map.base,
            kind: RobotKind::Scout,
            carrying: None,
        });
    }
    for _ in 0..2 {
        robots.push(Robot {
            pos: map.base,
            kind: RobotKind::Collector,
            carrying: None,
        });
    }

    let kinds: Vec<RobotKind> = robots.iter().map(|r| r.kind).collect();

    let state = Arc::new(Mutex::new(WorldState {
        resources: initial_resources,
        known: HashMap::new(),
        robots,
        totals: (0, 0),
    }));

    let (tx, rx) = mpsc::channel::<RobotEvent>();

    for (id, kind) in kinds.into_iter().enumerate() {
        let tx_c = tx.clone();
        let map_c = Arc::clone(&map);
        let state_c = Arc::clone(&state);
        thread::spawn(move || match kind {
            RobotKind::Scout => run_scout(id, map_c, state_c, tx_c),
            RobotKind::Collector => run_collector(id, map_c, state_c, tx_c),
        });
    }
    drop(tx); // main no longer sends events

    loop {
        // Drain any pending events and update aggregated state
        loop {
            match rx.try_recv() {
                Ok(event) => {
                    let mut s = state.lock().unwrap();
                    match event {
                        RobotEvent::Discovered { pos, kind } => {
                            s.known.entry(pos).or_insert(kind);
                        }
                        RobotEvent::Collected { kind, amount } => match kind {
                            ResourceKind::Energy => s.totals.0 += amount,
                            ResourceKind::Crystal => s.totals.1 += amount,
                        },
                    }
                }
                Err(_) => break,
            }
        }

        // Snapshot for rendering, then release the lock before drawing
        let snap = {
            let s = state.lock().unwrap();
            (s.robots.clone(), s.resources.clone(), s.totals)
        };

        terminal.draw(|f| ui(f, &map, &snap.0, &snap.1, snap.2))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }
    }
}
