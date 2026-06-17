//! La carte mentale d'UN robot.
//!
//! Chaque robot possède la sienne : il démarre en ne connaissant que les abords
//! de la base, puis la complète en explorant (perception) et en recevant les
//! découvertes des autres (messages). C'est ce qui rend les robots réellement
//! indépendants : deux robots peuvent avoir des connaissances différentes.

use std::collections::HashMap;

use crate::map::Map;
use crate::world::{Cell, Pos, ResourceKind};

pub(crate) struct Knowledge {
    /// Terrain connu, `Unknown` au départ partout.
    pub(crate) cells: Vec<Vec<Cell>>,
    /// Gisements repérés (sans la quantité : seule la base/le monde la connaît).
    pub(crate) resources: HashMap<Pos, ResourceKind>,
}

impl Knowledge {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![vec![Cell::Unknown; width]; height],
            resources: HashMap::new(),
        }
    }

    /// Le robot connaît déjà le carré 3x3 autour de la base : c'est de là qu'il
    /// part, donc il en voit le terrain immédiat.
    pub(crate) fn with_base_seen(map: &Map) -> Self {
        let mut k = Self::new(map.width, map.height);
        for (p, cell) in around(map.base, map, 1) {
            k.note_cell(p, cell);
        }
        k
    }

    /// Une case inconnue est supposée franchissable : on le vérifiera en y allant.
    pub(crate) fn passable(&self, p: Pos) -> bool {
        self.cells[p.1][p.0] != Cell::Obstacle
    }

    pub(crate) fn note_cell(&mut self, p: Pos, cell: Cell) {
        self.cells[p.1][p.0] = cell;
    }

    pub(crate) fn note_resource(&mut self, p: Pos, kind: ResourceKind) {
        self.resources.insert(p, kind);
    }

    pub(crate) fn forget_resource(&mut self, p: &Pos) {
        self.resources.remove(p);
    }
}

/// Liste les cases (et leur terrain réel) dans un rayon `radius` autour de `pos`,
/// en restant dans la carte. Sert à percevoir l'environnement immédiat.
pub(crate) fn around(pos: Pos, map: &Map, radius: i32) -> Vec<(Pos, Cell)> {
    let mut out = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = pos.0 as i32 + dx;
            let y = pos.1 as i32 + dy;
            if x < 0 || y < 0 {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            if x >= map.width || y >= map.height {
                continue;
            }
            let cell = if map.passable((x, y)) {
                Cell::Free
            } else {
                Cell::Obstacle
            };
            out.push(((x, y), cell));
        }
    }
    out
}
