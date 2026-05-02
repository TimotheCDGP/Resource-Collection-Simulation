use std::collections::{HashMap, VecDeque};
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
use noise::{NoiseFn, Perlin};
use rand::Rng;
use rand::seq::SliceRandom;

mod world;
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

// ---------------- TERRAIN ----------------

#[derive(Clone)]
enum Tile {
    Empty,
    Obstacle,
    Base,
}

struct Map {
    width: usize,
    height: usize,
    tiles: Vec<Vec<Tile>>,
    base: (usize, usize),
}

impl Map {
    fn new(width: usize, height: usize) -> (Self, HashMap<(usize, usize), (ResourceKind, u32)>) {
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

    fn passable(&self, pos: (usize, usize)) -> bool {
        !matches!(self.tiles[pos.1][pos.0], Tile::Obstacle)
    }
}

// ---------------- MOVEMENT HELPERS ----------------

fn random_step(from: (usize, usize), map: &Map, rng: &mut impl Rng) -> (usize, usize) {
    let (x, y) = from;
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
        if !map.passable((nx, ny)) {
            continue;
        }
        return (nx, ny);
    }
    from
}

// BFS depuis `from` : retourne pour chaque cellule atteignable le premier pas à
// faire depuis `from` pour s'y rendre via un plus court chemin.
fn bfs_first_steps(
    from: (usize, usize),
    map: &Map,
) -> HashMap<(usize, usize), (usize, usize)> {
    let mut first_step: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut queue: VecDeque<((usize, usize), (usize, usize))> = VecDeque::new();
    first_step.insert(from, from);
    queue.push_back((from, from));

    while let Some((p, fs)) = queue.pop_front() {
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let nx = p.0 as i32 + dx;
            let ny = p.1 as i32 + dy;
            if nx < 0 || ny < 0 {
                continue;
            }
            let (nxu, nyu) = (nx as usize, ny as usize);
            if nxu >= map.width || nyu >= map.height {
                continue;
            }
            let np = (nxu, nyu);
            if !map.passable(np) {
                continue;
            }
            if first_step.contains_key(&np) {
                continue;
            }
            let new_fs = if p == from { np } else { fs };
            first_step.insert(np, new_fs);
            queue.push_back((np, new_fs));
        }
    }
    first_step
}

// ---------------- ROBOT THREAD BODIES ----------------

fn run_scout(
    id: usize,
    map: Arc<Map>,
    state: Arc<Mutex<WorldState>>,
    tx: mpsc::Sender<RobotEvent>,
) {
    let mut rng = rand::thread_rng();
    loop {
        let cur_pos = {
            let s = state.lock().unwrap();
            s.robots[id].pos
        };
        let new_pos = random_step(cur_pos, &map, &mut rng);

        let discovery = {
            let mut s = state.lock().unwrap();
            s.robots[id].pos = new_pos;
            s.resources.get(&new_pos).map(|&(k, _)| k)
        };

        if let Some(kind) = discovery {
            let _ = tx.send(RobotEvent::Discovered { pos: new_pos, kind });
        }

        thread::sleep(Duration::from_millis(150));
    }
}

fn run_collector(
    id: usize,
    map: Arc<Map>,
    state: Arc<Mutex<WorldState>>,
    tx: mpsc::Sender<RobotEvent>,
) {
    let mut rng = rand::thread_rng();
    let base = map.base;
    loop {
        let (cur_pos, carrying, known_positions) = {
            let s = state.lock().unwrap();
            let me = &s.robots[id];
            let known: Vec<(usize, usize)> = s
                .known
                .keys()
                .filter(|&&p| s.resources.contains_key(&p))
                .copied()
                .collect();
            (me.pos, me.carrying, known)
        };

        // 1. Carrying & at base → deposit
        if carrying.is_some() && cur_pos == base {
            let kind = carrying.unwrap();
            {
                let mut s = state.lock().unwrap();
                s.robots[id].carrying = None;
            }
            let _ = tx.send(RobotEvent::Collected { kind, amount: 1 });
            thread::sleep(Duration::from_millis(150));
            continue;
        }

        // 2. Empty hands & on a resource → pick up one unit
        if carrying.is_none() {
            let picked = {
                let mut s = state.lock().unwrap();
                if let Some(&(kind, qty)) = s.resources.get(&cur_pos) {
                    if qty > 0 {
                        let new_qty = qty - 1;
                        if new_qty == 0 {
                            s.resources.remove(&cur_pos);
                            s.known.remove(&cur_pos);
                        } else {
                            s.resources.insert(cur_pos, (kind, new_qty));
                        }
                        s.robots[id].carrying = Some(kind);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            if picked {
                thread::sleep(Duration::from_millis(150));
                continue;
            }
        }

        // 3. BFS depuis ma position : trouve le premier pas vers la cible la plus proche
        //    réellement atteignable (et non simplement à courte distance Manhattan).
        let first_steps = bfs_first_steps(cur_pos, &map);
        let target = if carrying.is_some() {
            if first_steps.contains_key(&base) {
                Some(base)
            } else {
                None
            }
        } else {
            let mut reachable: Vec<(usize, usize)> = known_positions
                .into_iter()
                .filter(|p| first_steps.contains_key(p))
                .collect();
            reachable.sort_by_key(|p| {
                let dx = p.0 as i32 - cur_pos.0 as i32;
                let dy = p.1 as i32 - cur_pos.1 as i32;
                dx.abs() + dy.abs()
            });
            reachable.first().copied()
        };

        let new_pos = match target {
            Some(t) if t != cur_pos => {
                first_steps.get(&t).copied().unwrap_or(cur_pos)
            }
            _ => random_step(cur_pos, &map, &mut rng),
        };
        {
            let mut s = state.lock().unwrap();
            s.robots[id].pos = new_pos;
        }

        thread::sleep(Duration::from_millis(150));
    }
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
