//! Rendu Ratatui : header (compteurs) et grille fusionnant la carte de
//! connaissance (brouillard), les ressources découvertes et les robots. Le
//! terrain non encore exploré reste masqué.

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::map::Map;
use crate::world::{Knowledge, ResourceKind, Robot, RobotKind};

pub(crate) fn ui(
    f: &mut Frame,
    map: &Map,
    robots: &[Robot],
    known_map: &[Vec<Knowledge>],
    known_resources: &HashMap<(usize, usize), ResourceKind>,
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
        "Resource Simulation  Energy:{}  Crystals:{}  (press any key to quit)",
        totals.0, totals.1
    );
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(header, chunks[0]);

    // Terrain depuis la connaissance de la base : l'inconnu reste masqué.
    let mut grid: Vec<Vec<(char, Style)>> =
        vec![vec![(' ', Style::default()); map.width]; map.height];
    for y in 0..map.height {
        for x in 0..map.width {
            grid[y][x] = match known_map[y][x] {
                Knowledge::Unknown => ('.', Style::default().fg(Color::DarkGray)),
                Knowledge::Obstacle => ('O', Style::default().fg(Color::LightCyan)),
                Knowledge::Free => {
                    if (x, y) == map.base {
                        ('#', Style::default().fg(Color::LightGreen))
                    } else {
                        (' ', Style::default())
                    }
                }
            };
        }
    }
    // Ressources découvertes uniquement.
    for (&(x, y), &kind) in known_resources {
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
