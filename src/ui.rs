//! Rendu Ratatui : header (compteurs) et grille fusionnant terrain, ressources
//! et robots. Le rendu est piloté par un snapshot pour éviter de bloquer les
//! threads pendant le draw.

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::map::{Map, Tile};
use crate::world::{ResourceKind, Robot, RobotKind};

pub(crate) fn ui(
    f: &mut Frame,
    map: &Map,
    robots: &[Robot],
    resources: &HashMap<(usize, usize), (ResourceKind, u32)>,
    totals: (u32, u32),
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(f.size());

    let header_text = format!(
        "Resource Simulation  Energy:{}  Crystals:{}  (q to quit)",
        totals.0, totals.1
    );
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(header, chunks[0]);

    let mut grid: Vec<Vec<(char, Style)>> =
        vec![vec![(' ', Style::default()); map.width]; map.height];
    for y in 0..map.height {
        for x in 0..map.width {
            grid[y][x] = match map.tiles[y][x] {
                Tile::Empty => (' ', Style::default()),
                Tile::Obstacle => ('O', Style::default().fg(Color::LightCyan)),
                Tile::Base => ('#', Style::default().fg(Color::LightGreen)),
            };
        }
    }
    for (&(x, y), &(kind, _)) in resources {
        let (sym, color) = match kind {
            ResourceKind::Energy => ('E', Color::Green),
            ResourceKind::Crystal => ('C', Color::LightMagenta),
        };
        grid[y][x] = (sym, Style::default().fg(color));
    }
    for robot in robots {
        let (x, y) = robot.pos;
        let (sym, color) = match robot.kind {
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

    let map_widget = Paragraph::new(lines).block(
        Block::default()
            .title("Map")
            .borders(Borders::ALL),
    );
    f.render_widget(map_widget, chunks[1]);
}
