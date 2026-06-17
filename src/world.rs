//! Types partagés du monde.
//!
//! Idée directrice : on sépare clairement deux choses.
//! - Le **monde physique** ([`World`]) : la seule vérité terrain, partagée
//!   derrière un `Mutex`. On n'y met QUE ce qui doit rester cohérent entre
//!   robots — leurs positions réelles et les stocks de ressources.
//! - La **connaissance** : elle n'est PAS ici. Chaque robot a sa propre carte
//!   mentale (voir [`crate::knowledge`]) et la partage avec les autres par
//!   messages ([`Report`] vers la base, [`News`] de la base vers les robots).

use std::collections::HashMap;

/// Coordonnée d'une case `(x, y)`.
pub(crate) type Pos = (usize, usize);

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

/// Ce qu'un robot sait d'une case. `Unknown` = jamais perçue.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cell {
    Unknown,
    Free,
    Obstacle,
}

/// Cases perçues (terrain) et gisements perçus : ce qui circule entre robots.
pub(crate) type SeenCells = Vec<(Pos, Cell)>;
pub(crate) type SeenResources = Vec<(Pos, ResourceKind)>;
pub(crate) type GoneResources = Vec<Pos>;

/// Monde physique partagé. Volontairement minimal : pas de connaissance ici,
/// seulement la réalité que plusieurs robots doivent modifier de façon cohérente.
pub(crate) struct World {
    /// Position réelle de chaque robot (l'index est l'id du robot).
    pub(crate) positions: Vec<Pos>,
    /// Gisements restants : case -> (type, quantité).
    pub(crate) resources: HashMap<Pos, (ResourceKind, u32)>,
}

/// Message d'un robot vers la base (communication asynchrone).
pub(crate) enum Report {
    /// Cases et ressources que le robot vient de découvrir (terrain + gisements).
    Seen {
        cells: SeenCells,
        resources: SeenResources,
        gone_resources: GoneResources,
    },
    /// Une unité a été déposée à la base.
    Delivered(ResourceKind),
}

/// Message de la base vers les robots : la connaissance fraîchement agrégée,
/// rediffusée pour que chacun profite des découvertes des autres.
#[derive(Clone)]
pub(crate) struct News {
    pub(crate) cells: SeenCells,
    pub(crate) resources: SeenResources,
    pub(crate) gone_resources: GoneResources,
}
