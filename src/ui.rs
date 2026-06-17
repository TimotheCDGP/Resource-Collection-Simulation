//! Rendu Ratatui : un bandeau (compteurs + aide) et la grille fusionnant la
//! connaissance agrégée de la base (brouillard), les gisements découverts et les
//! robots. Le terrain jamais exploré reste masqué.

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::map::Map;
use crate::world::{Cell, Pos, ResourceKind, RobotKind};

pub(crate) fn ui(
    f: &mut Frame,
    map: &Map,
    cells: &[Vec<Cell>],
    resources: &HashMap<Pos, ResourceKind>,
    positions: &[Pos],
    kinds: &[RobotKind],
    totals: (u32, u32),
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(f.size());

    let header = Paragraph::new(format!(
        "Resource Simulation  Energy:{}  Crystals:{}  (press any key to quit)",
        totals.0, totals.1
    ))
    .block(Block::default().borders(Borders::NONE));
    f.render_widget(header, chunks[0]);

    // Terrain : l'inconnu reste masqué ('.'), la base est marquée '#'.
    let mut grid: Vec<Vec<(char, Style)>> =
        vec![vec![(' ', Style::default()); map.width]; map.height];
    for y in 0..map.height {
        for x in 0..map.width {
            grid[y][x] = match cells[y][x] {
                Cell::Unknown => ('.', Style::default().fg(Color::DarkGray)),
                Cell::Obstacle => ('O', Style::default().fg(Color::LightCyan)),
                Cell::Free if (x, y) == map.base => {
                    ('#', Style::default().fg(Color::LightGreen))
                }
                Cell::Free => (' ', Style::default()),
            };
        }
    }

    // Gisements découverts (et encore présents).
    for (&(x, y), &kind) in resources {
        let (sym, color) = match kind {
            ResourceKind::Energy => ('E', Color::Green),
            ResourceKind::Crystal => ('C', Color::LightMagenta),
        };
        grid[y][x] = (sym, Style::default().fg(color));
    }

    // Robots par-dessus.
    for (&(x, y), &kind) in positions.iter().zip(kinds) {
        let (sym, color) = match kind {
            RobotKind::Scout => ('x', Color::Red),
            RobotKind::Collector => ('o', Color::Magenta),
        };
        grid[y][x] = (sym, Style::default().fg(color));
    }

    let lines: Vec<Line> = grid
        .iter()
        .map(|row| {
            Line::from(
                row.iter()
                    .map(|(c, s)| Span::styled(c.to_string(), *s))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let map_widget =
        Paragraph::new(lines).block(Block::default().title("Map").borders(Borders::ALL));
    f.render_widget(map_widget, chunks[1]);
}
