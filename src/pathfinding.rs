//! Helpers de navigation sur la `Map` : marche aléatoire évitant les obstacles
//! et BFS calculant le premier pas du plus court chemin vers chaque cellule
//! atteignable.

use std::collections::{HashMap, HashSet, VecDeque};

use rand::Rng;
use rand::seq::SliceRandom;

use crate::map::Map;

/// Pas aléatoire vers une case voisine traversable et non occupée par un autre
/// robot. Si aucune case libre n'est disponible, le robot reste sur place.
pub(crate) fn random_step(
    from: (usize, usize),
    map: &Map,
    occupied: &HashSet<(usize, usize)>,
    rng: &mut impl Rng,
) -> (usize, usize) {
    let (x, y) = from;
    let mut dirs: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    dirs.shuffle(rng);
    for (dx, dy) in dirs {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || ny < 0 {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        if nx >= map.width || ny >= map.height {
            continue;
        }
        if !map.passable((nx, ny)) {
            continue;
        }
        if occupied.contains(&(nx, ny)) {
            continue;
        }
        return (nx, ny);
    }
    from
}

/// BFS depuis `from` : retourne pour chaque cellule atteignable le premier pas
/// à faire depuis `from` pour s'y rendre via un plus court chemin.
pub(crate) fn bfs_first_steps(
    from: (usize, usize),
    map: &Map,
) -> HashMap<(usize, usize), (usize, usize)> {
    let mut first_step: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut queue: VecDeque<((usize, usize), (usize, usize))> = VecDeque::new();
    first_step.insert(from, from);
    queue.push_back((from, from));

    while let Some((p, fs)) = queue.pop_front() {
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let nx = p.0 as i32 + dx;
            let ny = p.1 as i32 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (nxu, nyu) = (nx as usize, ny as usize);
            if nxu >= map.width || nyu >= map.height {
                continue;
            }
            let np = (nxu, nyu);
            if !map.passable(np) {
                continue;
            }
            if first_step.contains_key(&np) {
                continue;
            }
            let new_fs = if p == from { np } else { fs };
            first_step.insert(np, new_fs);
            queue.push_back((np, new_fs));
        }
    }
    first_step
}
