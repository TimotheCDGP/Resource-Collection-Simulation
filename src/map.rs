//! Terrain immuable : génération procédurale via bruit de Perlin et tests de
//! traversabilité. Les ressources initiales sont retournées séparément ; la
//! `Map` ne porte que le terrain.

use std::collections::HashMap;

use noise::{NoiseFn, Perlin};
use rand::Rng;
use rand::seq::SliceRandom;

use crate::world::ResourceKind;

#[derive(Clone)]
pub(crate) enum Tile {
    Empty,
    Obstacle,
    Base,
}

pub(crate) struct Map {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) tiles: Vec<Vec<Tile>>,
    pub(crate) base: (usize, usize),
}

impl Map {
    pub(crate) fn new(
        width: usize,
        height: usize,
    ) -> (Self, HashMap<(usize, usize), (ResourceKind, u32)>) {
        let mut rng = rand::thread_rng();
        let perlin = Perlin::new(rng.r#gen());

        let mut tiles = vec![vec![Tile::Empty; width]; height];

        for y in 0..height {
            for x in 0..width {
                let v = perlin.get([x as f64 * 0.12, y as f64 * 0.12]);
                if v > 0.2 {
                    tiles[y][x] = Tile::Obstacle;
                }
            }
        }

        let bx = width / 2;
        let by = height / 2;
        // Garantit une zone 3x3 dégagée autour de la base pour le spawn
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let nx = bx as i32 + dx;
                let ny = by as i32 + dy;
                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    tiles[ny as usize][nx as usize] = Tile::Empty;
                }
            }
        }
        tiles[by][bx] = Tile::Base;

        let mut empty: Vec<(usize, usize)> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .filter(|&(x, y)| matches!(tiles[y][x], Tile::Empty))
            .collect();
        empty.shuffle(&mut rng);

        let mut resources: HashMap<(usize, usize), (ResourceKind, u32)> = HashMap::new();

        let energy_count = rng.gen_range(5..=10).min(empty.len());
        for _ in 0..energy_count {
            if let Some(pos) = empty.pop() {
                resources.insert(pos, (ResourceKind::Energy, rng.gen_range(50..=200)));
            }
        }

        let crystal_count = rng.gen_range(5..=10).min(empty.len());
        for _ in 0..crystal_count {
            if let Some(pos) = empty.pop() {
                resources.insert(pos, (ResourceKind::Crystal, rng.gen_range(50..=200)));
            }
        }

        (
            Self {
                width,
                height,
                tiles,
                base: (bx, by),
            },
            resources,
        )
    }

    pub(crate) fn passable(&self, pos: (usize, usize)) -> bool {
        !matches!(self.tiles[pos.1][pos.0], Tile::Obstacle)
    }
}
