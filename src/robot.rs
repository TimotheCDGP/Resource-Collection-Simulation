//! Boucles de comportement exécutées sur un thread par robot. Les scouts
//! parcourent la carte et signalent leurs découvertes via le canal mpsc ;
//! les collecteurs cherchent la ressource connue la plus proche, la rapportent
//! à la base et émettent un événement de collecte.

use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::map::Map;
use crate::pathfinding::{bfs_first_steps, random_step};
use crate::world::{RobotEvent, ResourceKind, WorldState};

/// Rayon (en cases) dans lequel un éclaireur perçoit les ressources autour de
/// lui. 0 reviendrait à ne découvrir qu'en marchant exactement dessus.
const SCOUT_VISION: i32 = 2;

pub(crate) fn run_scout(
    id: usize,
    map: Arc<Map>,
    state: Arc<Mutex<WorldState>>,
    tx: mpsc::Sender<RobotEvent>,
) {
    let mut rng = rand::thread_rng();
    loop {
        // Choix du déplacement et scan de vision sous un seul lock : la case
        // visée est garantie libre au moment de l'écriture, donc deux robots
        // ne peuvent jamais se retrouver sur la même case.
        let discoveries: Vec<((usize, usize), ResourceKind)> = {
            let mut s = state.lock().unwrap();
            let cur_pos = s.robots[id].pos;
            let occupied: HashSet<(usize, usize)> = s
                .robots
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != id)
                .map(|(_, r)| r.pos)
                .collect();

            let new_pos = random_step(cur_pos, &map, &occupied, &mut rng);
            s.robots[id].pos = new_pos;

            // Perception locale : toute ressource présente dans le rayon de
            // vision est signalée.
            let mut found = Vec::new();
            for dy in -SCOUT_VISION..=SCOUT_VISION {
                for dx in -SCOUT_VISION..=SCOUT_VISION {
                    let cx = new_pos.0 as i32 + dx;
                    let cy = new_pos.1 as i32 + dy;
                    if cx < 0 || cy < 0 {
                        continue;
                    }
                    let cell = (cx as usize, cy as usize);
                    if let Some(&(kind, _)) = s.resources.get(&cell) {
                        found.push((cell, kind));
                    }
                }
            }
            found
        };

        for (pos, kind) in discoveries {
            let _ = tx.send(RobotEvent::Discovered { pos, kind });
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
        let (cur_pos, carrying, known_positions, occupied) = {
            let s = state.lock().unwrap();
            let me = &s.robots[id];
            let known: Vec<(usize, usize)> = s
                .known
                .keys()
                .filter(|&&p| s.resources.contains_key(&p))
                .copied()
                .collect();
            let occ: HashSet<(usize, usize)> = s
                .robots
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != id)
                .map(|(_, r)| r.pos)
                .collect();
            (me.pos, me.carrying, known, occ)
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
                let step = first_steps.get(&t).copied().unwrap_or(cur_pos);
                // Si le pas calculé est occupé par un autre robot, on attend
                // (on reste sur place) plutôt que d'entrer en collision.
                if occupied.contains(&step) { cur_pos } else { step }
            }
            _ => random_step(cur_pos, &map, &occupied, &mut rng),
        };
        {
            let mut s = state.lock().unwrap();
            // Re-vérification sous le lock d'écriture : garantit l'absence de
            // collision même si un autre robot a bougé depuis le snapshot.
            let busy = s
                .robots
                .iter()
                .enumerate()
                .any(|(i, r)| i != id && r.pos == new_pos);
            if !busy || new_pos == cur_pos {
                s.robots[id].pos = new_pos;
            }
        }

        thread::sleep(Duration::from_millis(150));
    }
}
