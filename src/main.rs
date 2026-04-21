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
enum Tile {
    Empty,
    Obstacle,
    Base,
}

struct Map {
    width: usize,
    height: usize,
    tiles: Vec<Vec<Tile>>,
}

impl Map {
    fn new(width: usize, height: usize) -> Self {
        let mut tiles = vec![vec![Tile::Empty; width]; height];

        for y in 0..height {
            for x in 0..width {
                let v = (x * 31 + y * 17) % 100;

                tiles[y][x] = if v < 12 {
                    Tile::Obstacle
                } else {
                    Tile::Empty
                };
            }
        }

        // Base centrale
        tiles[height / 2][width / 2] = Tile::Base;

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
                Tile::Obstacle => ('O', Style::default().fg(Color::Cyan)),
                Tile::Base => ('#', Style::default().fg(Color::Green)),
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