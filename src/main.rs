use std::collections::HashMap;
use std::io;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    Frame,
    layout::{Layout, Constraint, Direction},
    widgets::{Block, Borders, Paragraph},
    style::{Style, Color},
    text::{Line, Span},
};

mod map;
mod pathfinding;
mod robot;
mod world;
use crate::map::{Map, Tile};
use crate::robot::{run_collector, run_scout};
use crate::world::{ResourceKind, Robot, RobotEvent, RobotKind, WorldState};

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

// ---------------- APP LOOP ----------------

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
) -> io::Result<()> {
    let (map, initial_resources) = Map::new(80, 25);
    let map = Arc::new(map);

    let mut robots = Vec::new();
    for _ in 0..3 {
        robots.push(Robot {
            pos: map.base,
            kind: RobotKind::Scout,
            carrying: None,
        });
    }
    for _ in 0..2 {
        robots.push(Robot {
            pos: map.base,
            kind: RobotKind::Collector,
            carrying: None,
        });
    }

    let kinds: Vec<RobotKind> = robots.iter().map(|r| r.kind).collect();

    let state = Arc::new(Mutex::new(WorldState {
        resources: initial_resources,
        known: HashMap::new(),
        robots,
        totals: (0, 0),
    }));

    let (tx, rx) = mpsc::channel::<RobotEvent>();

    for (id, kind) in kinds.into_iter().enumerate() {
        let tx_c = tx.clone();
        let map_c = Arc::clone(&map);
        let state_c = Arc::clone(&state);
        thread::spawn(move || match kind {
            RobotKind::Scout => run_scout(id, map_c, state_c, tx_c),
            RobotKind::Collector => run_collector(id, map_c, state_c, tx_c),
        });
    }
    drop(tx); // main no longer sends events

    loop {
        // Drain any pending events and update aggregated state
        loop {
            match rx.try_recv() {
                Ok(event) => {
                    let mut s = state.lock().unwrap();
                    match event {
                        RobotEvent::Discovered { pos, kind } => {
                            s.known.entry(pos).or_insert(kind);
                        }
                        RobotEvent::Collected { kind, amount } => match kind {
                            ResourceKind::Energy => s.totals.0 += amount,
                            ResourceKind::Crystal => s.totals.1 += amount,
                        },
                    }
                }
                Err(_) => break,
            }
        }

        // Snapshot for rendering, then release the lock before drawing
        let snap = {
            let s = state.lock().unwrap();
            (s.robots.clone(), s.resources.clone(), s.totals)
        };

        terminal.draw(|f| ui(f, &map, &snap.0, &snap.1, snap.2))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }
    }
}

// ---------------- UI ----------------

fn ui(
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
