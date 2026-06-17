//! Navigation : marche aléatoire et BFS « premier pas ».
//!
//! Les deux fonctions sont génériques sur une closure qui décide si une case est
//! utilisable. Elles servent donc indifféremment sur le terrain réel (pour
//! bouger) ou sur la carte mentale d'un robot (pour planifier).

use std::collections::{HashMap, VecDeque};

use rand::seq::SliceRandom;
use rand::Rng;

use crate::world::Pos;

/// Pas aléatoire vers une case voisine telle que `free(case)` est vraie. Si
/// aucune voisine n'est libre, le robot reste sur place (renvoie `from`).
pub(crate) fn random_step(
    from: Pos,
    width: usize,
    height: usize,
    free: impl Fn(Pos) -> bool,
    rng: &mut impl Rng,
) -> Pos {
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
        if nx >= width || ny >= height {
            continue;
        }
        if free((nx, ny)) {
            return (nx, ny);
        }
    }
    from
}

/// BFS depuis `from`. `passable` décide de la franchissabilité de chaque case.
/// Retourne, pour chaque case atteignable, le premier pas à faire depuis `from`
/// pour s'y rendre par un plus court chemin.
pub(crate) fn bfs_first_steps(
    from: Pos,
    width: usize,
    height: usize,
    passable: impl Fn(Pos) -> bool,
) -> HashMap<Pos, Pos> {
    let mut first_step: HashMap<Pos, Pos> = HashMap::new();
    let mut queue: VecDeque<(Pos, Pos)> = VecDeque::new();
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
            if nxu >= width || nyu >= height {
                continue;
            }
            let np = (nxu, nyu);
            if !passable(np) || first_step.contains_key(&np) {
                continue;
            }
            // Le premier pas est la case elle-même si on part de `from`,
            // sinon on hérite du premier pas de la case d'où l'on vient.
            let new_fs = if p == from { np } else { fs };
            first_step.insert(np, new_fs);
            queue.push_back((np, new_fs));
        }
    }
    first_step
}
