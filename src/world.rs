//! Types de données partagés du monde simulé : ressources, robots, état global,
//! événements échangés entre threads.

use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ResourceKind {
    Energy,
    Crystal,
}

#[derive(Clone, Copy)]
pub(crate) enum RobotKind {
    Scout,
    Collector,
}

/// Connaissance d'une cellule par la base. Les robots démarrent en ne
/// connaissant que les abords immédiats de la base ; le reste est `Unknown`
/// jusqu'à ce qu'un robot le perçoive.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Knowledge {
    Unknown,
    Free,
    Obstacle,
}

#[derive(Clone)]
pub(crate) struct Robot {
    pub(crate) pos: (usize, usize),
    pub(crate) kind: RobotKind,
    pub(crate) carrying: Option<ResourceKind>,
}

pub(crate) struct WorldState {
    pub(crate) resources: HashMap<(usize, usize), (ResourceKind, u32)>,
    pub(crate) known: HashMap<(usize, usize), ResourceKind>,
    pub(crate) known_map: Vec<Vec<Knowledge>>,
    pub(crate) robots: Vec<Robot>,
    pub(crate) totals: (u32, u32),
}

pub(crate) enum RobotEvent {
    Discovered { pos: (usize, usize), kind: ResourceKind },
    Collected { kind: ResourceKind, amount: u32 },
}
