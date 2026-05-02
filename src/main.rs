use std::io;
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
use noise::{NoiseFn, Perlin};
use rand::Rng;
use rand::seq::SliceRandom;

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

// ---------------- MAP ----------------

#[derive(Clone)]
#[allow(dead_code)]
enum Tile {
    Empty,
    Obstacle,
    Base,
    Energy(u32),
    Crystal(u32),
}

struct Map {
    width: usize,
    height: usize,
    tiles: Vec<Vec<Tile>>,
}

impl Map {
    fn new(width: usize, height: usize) -> Self {
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

        let energy_count = rng.gen_range(5..=10).min(empty.len());
        for _ in 0..energy_count {
            if let Some((x, y)) = empty.pop() {
                tiles[y][x] = Tile::Energy(rng.gen_range(50..=200));
            }
        }

        let crystal_count = rng.gen_range(5..=10).min(empty.len());
        for _ in 0..crystal_count {
            if let Some((x, y)) = empty.pop() {
                tiles[y][x] = Tile::Crystal(rng.gen_range(50..=200));
            }
        }

        Self {
            width,
            height,
            tiles,
        }
    }
}

// ---------------- APP LOOP ----------------

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
) -> io::Result<()> {
    let map = Map::new(80, 25);

    loop {
        terminal.draw(|f| ui(f, &map))?;

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

fn ui(f: &mut Frame, map: &Map) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header minimal
            Constraint::Min(0),    // map prend tout le reste
        ])
        .split(f.size());

    // HEADER
    let header = Paragraph::new("Resource Simulation - Step 2 (q to quit)")
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(header, chunks[0]);

    // MAP RENDER (cell by cell)
    let mut lines: Vec<Line> = Vec::new();

    for y in 0..map.height {
        let mut spans: Vec<Span> = Vec::new();

        for x in 0..map.width {
            let (symbol, style) = match map.tiles[y][x] {
                Tile::Empty => (' ', Style::default()),
                Tile::Obstacle => ('O', Style::default().fg(Color::LightCyan)),
                Tile::Base => ('#', Style::default().fg(Color::LightGreen)),
                Tile::Energy(_) => ('E', Style::default().fg(Color::Green)),
                Tile::Crystal(_) => ('C', Style::default().fg(Color::LightMagenta)),
            };

            spans.push(Span::styled(symbol.to_string(), style));
        }

        lines.push(Line::from(spans));
    }

    let map_widget = Paragraph::new(lines).block(
        Block::default()
            .title("Map")
            .borders(Borders::ALL),
    );

    f.render_widget(map_widget, chunks[1]);
}