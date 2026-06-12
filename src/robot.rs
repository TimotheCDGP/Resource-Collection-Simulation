//! Boucles de comportement exécutées sur un thread par robot. Les scouts
//! parcourent la carte et signalent leurs découvertes via le canal mpsc ;
//! les collecteurs cherchent la ressource connue la plus proche, la rapportent
//! à la base et émettent un événement de collecte.

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::map::Map;
use crate::pathfinding::{bfs_first_steps, random_step};
use crate::world::{Knowledge, RobotEvent, WorldState};

/// Met à jour la connaissance de la base pour les cellules adjacentes à `pos`
/// (rayon 1) à partir du terrain réel. Chaque robot perçoit ses abords
/// immédiats à chaque tick, ce qui dévoile progressivement la carte.
fn perceive(pos: (usize, usize), map: &Map, s: &mut WorldState) {
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let nx = pos.0 as i32 + dx;
            let ny = pos.1 as i32 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if nx >= map.width || ny >= map.height {
                continue;
            }
            let cell = (nx, ny);
            let res_here = s.resources.get(&cell).map(|&(k, _)| k);
            s.known_map[ny][nx] = if map.passable(cell) {
                Knowledge::Free
            } else {
                Knowledge::Obstacle
            };
            if let Some(kind) = res_here {
                s.known.entry(cell).or_insert(kind);
            }
        }
    }
}

pub(crate) fn run_scout(
    id: usize,
    map: Arc<Map>,
    state: Arc<Mutex<WorldState>>,
    tx: mpsc::Sender<RobotEvent>,
) {
    let mut rng = rand::thread_rng();
    loop {
        let cur_pos = {
            let mut s = state.lock().unwrap();
            let p = s.robots[id].pos;
            perceive(p, &map, &mut s);
            p
        };
        let new_pos = random_step(cur_pos, &map, &mut rng);

        let discovery = {
            let mut s = state.lock().unwrap();
            s.robots[id].pos = new_pos;
            s.resources.get(&new_pos).map(|&(k, _)| k)
        };

        if let Some(kind) = discovery {
            let _ = tx.send(RobotEvent::Discovered { pos: new_pos, kind });
        }

        thread::sleep(Duration::from_millis(150));
    }
}

pub(crate) fn run_collector(
    id: usize,
    map: Arc<Map>,
    state: Arc<Mutex<WorldState>>,
    tx: mpsc::Sender<RobotEvent>,
) {
    let mut rng = rand::thread_rng();
    let base = map.base;
    loop {
        let (cur_pos, carrying, known_positions) = {
            let mut s = state.lock().unwrap();
            let pos = s.robots[id].pos;
            perceive(pos, &map, &mut s);
            let carrying = s.robots[id].carrying;
            let known: Vec<(usize, usize)> = s
                .known
                .keys()
                .filter(|&&p| s.resources.contains_key(&p))
                .copied()
                .collect();
            (pos, carrying, known)
        };

        // 1. Carrying & at base → deposit
        if carrying.is_some() && cur_pos == base {
            let kind = carrying.unwrap();
            {
                let mut s = state.lock().unwrap();
                s.robots[id].carrying = None;
            }
            let _ = tx.send(RobotEvent::Collected { kind, amount: 1 });
            thread::sleep(Duration::from_millis(150));
            continue;
        }

        // 2. Empty hands & on a resource → pick up one unit
        if carrying.is_none() {
            let picked = {
                let mut s = state.lock().unwrap();
                if let Some(&(kind, qty)) = s.resources.get(&cur_pos) {
                    if qty > 0 {
                        let new_qty = qty - 1;
                        if new_qty == 0 {
                            s.resources.remove(&cur_pos);
                            s.known.remove(&cur_pos);
                        } else {
                            s.resources.insert(cur_pos, (kind, new_qty));
                        }
                        s.robots[id].carrying = Some(kind);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            if picked {
                thread::sleep(Duration::from_millis(150));
                continue;
            }
        }

        // 3. BFS depuis ma position : trouve le premier pas vers la cible la plus proche
        //    réellement atteignable (et non simplement à courte distance Manhattan).
        let first_steps = bfs_first_steps(cur_pos, &map);
        let target = if carrying.is_some() {
            if first_steps.contains_key(&base) {
                Some(base)
            } else {
                None
            }
        } else {
            let mut reachable: Vec<(usize, usize)> = known_positions
                .into_iter()
                .filter(|p| first_steps.contains_key(p))
                .collect();
            reachable.sort_by_key(|p| {
                let dx = p.0 as i32 - cur_pos.0 as i32;
                let dy = p.1 as i32 - cur_pos.1 as i32;
                dx.abs() + dy.abs()
            });
            reachable.first().copied()
        };

        let new_pos = match target {
            Some(t) if t != cur_pos => {
                first_steps.get(&t).copied().unwrap_or(cur_pos)
            }
            _ => random_step(cur_pos, &map, &mut rng),
        };
        {
            let mut s = state.lock().unwrap();
            s.robots[id].pos = new_pos;
        }

        thread::sleep(Duration::from_millis(150));
    }
}
