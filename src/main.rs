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
    base: (usize, usize),
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
            base: (bx, by),
        }
    }
}

// ---------------- ROBOTS ----------------

#[derive(Clone, Copy)]
enum RobotKind {
    Scout,
    Collector,
}

struct Robot {
    pos: (usize, usize),
    kind: RobotKind,
}

impl Robot {
    fn new(kind: RobotKind, pos: (usize, usize)) -> Self {
        Self { kind, pos }
    }

    fn step(&mut self, map: &Map, rng: &mut impl Rng) {
        let (x, y) = self.pos;
        let mut dirs: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        dirs.shuffle(rng);
        for (dx, dy) in dirs {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if nx >= map.width || ny >= map.height {
                continue;
            }
            if matches!(map.tiles[ny][nx], Tile::Obstacle) {
                continue;
            }
            self.pos = (nx, ny);
            return;
        }
    }
}

// ---------------- APP LOOP ----------------

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
) -> io::Result<()> {
    let mut rng = rand::thread_rng();
    let map = Map::new(80, 25);

    let mut robots: Vec<Robot> = Vec::new();
    for _ in 0..3 {
        robots.push(Robot::new(RobotKind::Scout, map.base));
    }
    for _ in 0..2 {
        robots.push(Robot::new(RobotKind::Collector, map.base));
    }

    let totals = (0u32, 0u32); // (energy, crystals) — alimenté au commit 3

    loop {
        for robot in &mut robots {
            robot.step(&map, &mut rng);
        }

        terminal.draw(|f| ui(f, &map, &robots, totals))?;

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

fn ui(f: &mut Frame, map: &Map, robots: &[Robot], totals: (u32, u32)) {
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

    // Build display grid: tiles first, robots overlay on top
    let mut grid: Vec<Vec<(char, Style)>> =
        vec![vec![(' ', Style::default()); map.width]; map.height];
    for y in 0..map.height {
        for x in 0..map.width {
            grid[y][x] = match map.tiles[y][x] {
                Tile::Empty => (' ', Style::default()),
                Tile::Obstacle => ('O', Style::default().fg(Color::LightCyan)),
                Tile::Base => ('#', Style::default().fg(Color::LightGreen)),
                Tile::Energy(_) => ('E', Style::default().fg(Color::Green)),
                Tile::Crystal(_) => ('C', Style::default().fg(Color::LightMagenta)),
            };
        }
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