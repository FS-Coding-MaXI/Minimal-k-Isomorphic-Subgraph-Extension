use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use itertools::Itertools;
use minimal_k_isomorphic_subgraph_extension::{
    cost::{calculate_edge_map, calculate_total_cost},
    mapping::find_all_mappings,
    parser::parse_input_file,
    utils::num_combinations,
    Graph, Mapping,
};
use rand::seq::SliceRandom;
use rand::thread_rng;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

/// Type alias for edge map: (source, target) -> edge count
type EdgeMap = HashMap<(usize, usize), usize>;

/// Algorithm type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Algorithm {
    Exact,
    Approx,
}

impl std::str::FromStr for Algorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "exact" => Ok(Algorithm::Exact),
            "approx" | "approximate" | "approximation" => Ok(Algorithm::Approx),
            _ => Err(format!("Invalid algorithm: {}. Use 'exact' or 'approx'", s)),
        }
    }
}

/// Progress messages from the algorithm thread
#[derive(Debug, Clone)]
enum ProgressMessage {
    Status(String),
    MappingProgress {
        current: usize,
        total: usize,
    },
    Complete {
        cost: usize,
        edge_map: EdgeMap,
        mappings: Vec<Mapping>,
        elapsed: Duration,
    },
    Error(String),
}

/// Command-line arguments
#[derive(Parser, Debug)]
#[command(author, version, about = "Unified Solver for Minimal k-Isomorphic Subgraph Extension", long_about = None)]
struct Args {
    /// Algorithm to use: 'exact' or 'approx'
    #[arg(short, long)]
    algorithm: Algorithm,

    /// Path to the input file containing graph descriptions
    #[arg(short, long)]
    input: PathBuf,

    /// Number of distinct isomorphic mappings required (k)
    #[arg(short, long)]
    k: usize,

    /// Output file path for results. Default: solution_{algorithm}.txt
    /// If not specified and graph has >15 vertices, output goes to file automatically.
    #[arg(short, long)]
    output_file: Option<PathBuf>,
}

/// Current view in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Calculating,
    Menu,
    Graphs,
    Extension,
    Mappings,
}

/// Viewport for scrolling large matrices
#[derive(Debug, Clone, Default)]
struct Viewport {
    row_offset: usize,
    col_offset: usize,
}

/// Application state
struct AppState {
    // Input data
    algorithm: Algorithm,
    g: Graph,
    h: Graph,
    k: usize,

    // Calculation state
    calculating: bool,
    start_time: Instant,
    status_message: String,
    current_mapping: usize,
    total_mappings: usize,
    spinner_frame: usize,

    // Results
    cost: Option<usize>,
    edge_map: Option<EdgeMap>,
    mappings: Option<Vec<Mapping>>,
    elapsed: Option<Duration>,

    // UI state
    current_view: View,
    selected_mapping: usize,
    viewport_g: Viewport,
    viewport_h: Viewport,
    viewport_ext: Viewport,
    viewport_mappings: Viewport,

    // File output
    output_file: Option<PathBuf>,

    // Progress channel
    progress_rx: Receiver<ProgressMessage>,
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl AppState {
    fn new(
        algorithm: Algorithm,
        g: Graph,
        h: Graph,
        k: usize,
        output_file: Option<PathBuf>,
        progress_rx: Receiver<ProgressMessage>,
    ) -> Self {
        Self {
            algorithm,
            g,
            h,
            k,
            calculating: true,
            start_time: Instant::now(),
            status_message: "Initializing...".to_string(),
            current_mapping: 0,
            total_mappings: k,
            spinner_frame: 0,
            cost: None,
            edge_map: None,
            mappings: None,
            elapsed: None,
            current_view: View::Calculating,
            selected_mapping: 0,
            viewport_g: Viewport::default(),
            viewport_h: Viewport::default(),
            viewport_ext: Viewport::default(),
            viewport_mappings: Viewport::default(),
            output_file,
            progress_rx,
        }
    }

    fn update(&mut self) -> io::Result<()> {
        // Advance spinner animation
        if self.calculating {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }

        // Check for progress messages (non-blocking)
        while let Ok(msg) = self.progress_rx.try_recv() {
            match msg {
                ProgressMessage::Status(status) => {
                    self.status_message = status;
                }
                ProgressMessage::MappingProgress { current, total } => {
                    self.current_mapping = current;
                    self.total_mappings = total;
                }
                ProgressMessage::Complete {
                    cost,
                    edge_map,
                    mappings,
                    elapsed,
                } => {
                    self.calculating = false;
                    self.cost = Some(cost);
                    self.edge_map = Some(edge_map.clone());
                    self.mappings = Some(mappings.clone());
                    self.elapsed = Some(elapsed);

                    // Save to file if output_file is set
                    if let Some(ref path) = self.output_file {
                        let _ = write_results_to_file(
                            path,
                            &self.g,
                            &self.h,
                            self.k,
                            self.algorithm,
                            cost,
                            &edge_map,
                            &mappings,
                            elapsed,
                        );
                    }

                    self.current_view = View::Menu;
                }
                ProgressMessage::Error(err) => {
                    self.status_message = format!("Error: {}", err);
                    self.calculating = false;
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyCode) {
        match self.current_view {
            View::Calculating => {}
            View::Menu => match key {
                KeyCode::Char('g') | KeyCode::Char('G') => self.current_view = View::Graphs,
                KeyCode::Char('e') | KeyCode::Char('E') => self.current_view = View::Extension,
                KeyCode::Char('v') | KeyCode::Char('V') => self.current_view = View::Mappings,
                _ => {}
            },
            View::Graphs => match key {
                KeyCode::Esc => self.current_view = View::Menu,
                KeyCode::Tab => {
                    // Tab switches between scrolling G and H (toggle focus)
                    // We use a simple swap of offsets to indicate focus change
                    std::mem::swap(&mut self.viewport_g, &mut self.viewport_h);
                    std::mem::swap(&mut self.viewport_g, &mut self.viewport_h);
                }
                KeyCode::Up => {
                    self.viewport_g.row_offset = self.viewport_g.row_offset.saturating_sub(1);
                    self.viewport_h.row_offset = self.viewport_h.row_offset.saturating_sub(1);
                }
                KeyCode::Down => {
                    if self.viewport_g.row_offset < self.g.num_vertices().saturating_sub(1) {
                        self.viewport_g.row_offset += 1;
                    }
                    if self.viewport_h.row_offset < self.h.num_vertices().saturating_sub(1) {
                        self.viewport_h.row_offset += 1;
                    }
                }
                KeyCode::Left => {
                    self.viewport_g.col_offset = self.viewport_g.col_offset.saturating_sub(1);
                    self.viewport_h.col_offset = self.viewport_h.col_offset.saturating_sub(1);
                }
                KeyCode::Right => {
                    if self.viewport_g.col_offset < self.g.num_vertices().saturating_sub(1) {
                        self.viewport_g.col_offset += 1;
                    }
                    if self.viewport_h.col_offset < self.h.num_vertices().saturating_sub(1) {
                        self.viewport_h.col_offset += 1;
                    }
                }
                KeyCode::Char('[') => {
                    self.viewport_g.col_offset = self.viewport_g.col_offset.saturating_sub(5);
                    self.viewport_h.col_offset = self.viewport_h.col_offset.saturating_sub(5);
                }
                KeyCode::Char(']') => {
                    self.viewport_g.col_offset = (self.viewport_g.col_offset + 5)
                        .min(self.g.num_vertices().saturating_sub(1));
                    self.viewport_h.col_offset = (self.viewport_h.col_offset + 5)
                        .min(self.h.num_vertices().saturating_sub(1));
                }
                KeyCode::PageUp => {
                    self.viewport_g.row_offset = self.viewport_g.row_offset.saturating_sub(10);
                    self.viewport_h.row_offset = self.viewport_h.row_offset.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.viewport_g.row_offset = (self.viewport_g.row_offset + 10)
                        .min(self.g.num_vertices().saturating_sub(1));
                    self.viewport_h.row_offset = (self.viewport_h.row_offset + 10)
                        .min(self.h.num_vertices().saturating_sub(1));
                }
                KeyCode::Home => {
                    self.viewport_g.row_offset = 0;
                    self.viewport_g.col_offset = 0;
                    self.viewport_h.row_offset = 0;
                    self.viewport_h.col_offset = 0;
                }
                KeyCode::End => {
                    self.viewport_g.row_offset = self.g.num_vertices().saturating_sub(1);
                    self.viewport_g.col_offset = self.g.num_vertices().saturating_sub(1);
                    self.viewport_h.row_offset = self.h.num_vertices().saturating_sub(1);
                    self.viewport_h.col_offset = self.h.num_vertices().saturating_sub(1);
                }
                _ => {}
            },
            View::Extension => match key {
                KeyCode::Esc => self.current_view = View::Menu,
                KeyCode::Up => {
                    self.viewport_ext.row_offset = self.viewport_ext.row_offset.saturating_sub(1)
                }
                KeyCode::Down => {
                    if self.viewport_ext.row_offset < self.h.num_vertices().saturating_sub(1) {
                        self.viewport_ext.row_offset += 1;
                    }
                }
                KeyCode::Left => {
                    self.viewport_ext.col_offset = self.viewport_ext.col_offset.saturating_sub(1)
                }
                KeyCode::Right => {
                    if self.viewport_ext.col_offset < self.h.num_vertices().saturating_sub(1) {
                        self.viewport_ext.col_offset += 1;
                    }
                }
                KeyCode::Char('[') => {
                    self.viewport_ext.col_offset = self.viewport_ext.col_offset.saturating_sub(5)
                }
                KeyCode::Char(']') => {
                    self.viewport_ext.col_offset = (self.viewport_ext.col_offset + 5)
                        .min(self.h.num_vertices().saturating_sub(1));
                }
                KeyCode::PageUp => {
                    self.viewport_ext.row_offset = self.viewport_ext.row_offset.saturating_sub(10)
                }
                KeyCode::PageDown => {
                    self.viewport_ext.row_offset = (self.viewport_ext.row_offset + 10)
                        .min(self.h.num_vertices().saturating_sub(1));
                }
                KeyCode::Home => {
                    self.viewport_ext.row_offset = 0;
                    self.viewport_ext.col_offset = 0;
                }
                KeyCode::End => {
                    self.viewport_ext.row_offset = self.h.num_vertices().saturating_sub(1);
                    self.viewport_ext.col_offset = self.h.num_vertices().saturating_sub(1);
                }
                _ => {}
            },
            View::Mappings => match key {
                KeyCode::Esc => self.current_view = View::Menu,
                KeyCode::Left => {
                    self.viewport_mappings.col_offset =
                        self.viewport_mappings.col_offset.saturating_sub(1);
                }
                KeyCode::Right => {
                    if self.viewport_mappings.col_offset < self.h.num_vertices().saturating_sub(1) {
                        self.viewport_mappings.col_offset += 1;
                    }
                }
                KeyCode::Up => {
                    self.viewport_mappings.row_offset =
                        self.viewport_mappings.row_offset.saturating_sub(1);
                }
                KeyCode::Down => {
                    if self.viewport_mappings.row_offset < self.g.num_vertices().saturating_sub(1) {
                        self.viewport_mappings.row_offset += 1;
                    }
                }
                KeyCode::Char('[') => {
                    self.viewport_mappings.col_offset =
                        self.viewport_mappings.col_offset.saturating_sub(5);
                }
                KeyCode::Char(']') => {
                    self.viewport_mappings.col_offset = (self.viewport_mappings.col_offset + 5)
                        .min(self.h.num_vertices().saturating_sub(1));
                }
                KeyCode::PageUp => {
                    self.viewport_mappings.row_offset =
                        self.viewport_mappings.row_offset.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.viewport_mappings.row_offset = (self.viewport_mappings.row_offset + 10)
                        .min(self.g.num_vertices().saturating_sub(1));
                }
                KeyCode::Home => {
                    self.viewport_mappings.row_offset = 0;
                    self.viewport_mappings.col_offset = 0;
                }
                KeyCode::End => {
                    self.viewport_mappings.row_offset = self.g.num_vertices().saturating_sub(1);
                    self.viewport_mappings.col_offset = self.h.num_vertices().saturating_sub(1);
                }
                KeyCode::Char(',') | KeyCode::Char('<') => {
                    // Previous mapping
                    if self.selected_mapping > 0 {
                        self.selected_mapping -= 1;
                        self.viewport_mappings.row_offset = 0;
                        self.viewport_mappings.col_offset = 0;
                    }
                }
                KeyCode::Char('.') | KeyCode::Char('>') => {
                    // Next mapping
                    if let Some(ref mappings) = self.mappings {
                        if self.selected_mapping < mappings.len() - 1 {
                            self.selected_mapping += 1;
                            self.viewport_mappings.row_offset = 0;
                            self.viewport_mappings.col_offset = 0;
                        }
                    }
                }
                _ => {}
            },
        }
    }
}

/// Render the calculating view
fn render_calculating(f: &mut Frame, app: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(0),
        ])
        .split(area);

    // Header
    let title = match app.algorithm {
        Algorithm::Exact => "Exact Solver for k-Isomorphic Subgraph Extension",
        Algorithm::Approx => "Approximation Solver for k-Isomorphic Subgraph Extension",
    };
    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Info section
    let elapsed = app.start_time.elapsed();
    let trials_info = if app.algorithm == Algorithm::Approx {
        format!(
            "\nTrials per mapping: {} (n₁ × n₂)",
            app.g.num_vertices() * app.h.num_vertices()
        )
    } else {
        String::new()
    };

    let spinner = SPINNER_FRAMES[app.spinner_frame];
    let info_text = format!(
        "Graph G (pattern): {} vertices\n\
        Graph H (host): {} vertices\n\
        Required distinct mappings (k): {}\n\
        Algorithm: {}{}\n\n\
        Status: {} {}\n\n\
        Finding mapping {}/{}...\n\n\
        Elapsed time: {:.3}s",
        app.g.num_vertices(),
        app.h.num_vertices(),
        app.k,
        match app.algorithm {
            Algorithm::Exact => "Exact",
            Algorithm::Approx => "Approximation",
        },
        trials_info,
        app.status_message,
        spinner,
        app.current_mapping.min(app.total_mappings),
        app.total_mappings,
        elapsed.as_secs_f64()
    );

    let info = Paragraph::new(info_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        );
    f.render_widget(info, chunks[1]);
}

/// Render the results menu
fn render_menu(f: &mut Frame, app: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(8),
        ])
        .split(area);

    // Header with solution type
    let title = match app.algorithm {
        Algorithm::Exact => "EXACT SOLUTION FOUND",
        Algorithm::Approx => "APPROXIMATE SOLUTION FOUND",
    };
    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        );
    f.render_widget(header, chunks[0]);

    // Results info
    let cost = app.cost.unwrap_or(0);
    let elapsed = app.elapsed.unwrap_or(Duration::from_secs(0));
    let mappings_count = app.mappings.as_ref().map(|m| m.len()).unwrap_or(0);

    let mut results_lines = vec![Line::from(Span::styled(
        format!(
            "Cost: {} edges  │  Time: {}ms  │  Mappings: {}",
            cost,
            elapsed.as_millis(),
            mappings_count
        ),
        Style::default().fg(Color::Yellow),
    ))];

    if let Some(ref path) = app.output_file {
        results_lines.push(Line::from(""));
        results_lines.push(Line::from(Span::styled(
            format!("✓ Results saved to: {}", path.display()),
            Style::default().fg(Color::Green),
        )));
    }

    let results = Paragraph::new(results_lines)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        );
    f.render_widget(results, chunks[1]);

    // Menu options
    let menu_items = vec![
        ListItem::new("  [G] View Graphs G and H adjacency matrices"),
        ListItem::new("  [E] View Extension (edges to add to H)"),
        ListItem::new(format!("  [V] View Mappings ({} found)", mappings_count)),
        ListItem::new(""),
        ListItem::new("  [Q] Quit"),
    ];

    let menu = List::new(menu_items)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Menu "),
        );
    f.render_widget(menu, chunks[2]);
}

/// Render the extension view (original+added format)
fn render_extension(f: &mut Frame, app: &AppState, area: Rect) {
    let n = app.h.num_vertices();
    let viewport = &app.viewport_ext;

    // Calculate visible rows/cols based on terminal size
    let rows_visible = (area.height.saturating_sub(10) as usize).max(5);
    let cols_visible = ((area.width.saturating_sub(8)) / 7).max(5) as usize; // 7 chars per col for "original+added"

    let max_row = viewport.row_offset + rows_visible.min(n - viewport.row_offset);
    let max_col = viewport.col_offset + cols_visible.min(n - viewport.col_offset);

    let edge_map = app.edge_map.as_ref().unwrap();

    // Build matrix text with "original+added" format
    let mut lines = vec![];
    lines.push(Line::from(Span::styled(
        "Format: original+added",
        Style::default().fg(Color::Yellow),
    )));
    lines.push(Line::from(""));

    // Header line with column numbers
    let mut header = String::from("     ");
    for col in viewport.col_offset..max_col {
        header.push_str(&format!("{:6}", col));
    }
    if max_col < n {
        header.push_str("   ...");
    }
    lines.push(Line::from(Span::styled(
        header,
        Style::default().fg(Color::Cyan),
    )));

    // Matrix rows
    for row in viewport.row_offset..max_row {
        let mut line_spans = vec![Span::styled(
            format!("{:3}│", row),
            Style::default().fg(Color::Cyan),
        )];

        for col in viewport.col_offset..max_col {
            let original = app.h.get_edge(row, col);
            let added = edge_map.get(&(row, col)).copied().unwrap_or(0);

            let text = if added > 0 {
                format!("{}+{}", original, added)
            } else {
                format!("{}", original)
            };

            let style = if added > 0 {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if original > 0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            line_spans.push(Span::styled(format!("{:>6}", text), style));
        }

        if max_col < n {
            line_spans.push(Span::styled("   ...", Style::default().fg(Color::DarkGray)));
        }

        lines.push(Line::from(line_spans));
    }

    if max_row < n {
        lines.push(Line::from(Span::styled(
            "  ...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Stats
    let total_added: usize = edge_map.values().sum();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("Total edges added: {}", total_added),
        Style::default().fg(Color::Gray),
    )));

    // Navigation info under the matrix
    if n > rows_visible || n > cols_visible {
        lines.push(Line::from(Span::styled(
            format!(
                "Viewing rows {}-{}, cols {}-{} of {}x{}",
                viewport.row_offset,
                max_row.saturating_sub(1),
                viewport.col_offset,
                max_col.saturating_sub(1),
                n,
                n
            ),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "[↑↓←→] Scroll  [PgUp/Dn] Jump rows  [[/]] Jump cols  [Home/End] First/Last",
            Style::default().fg(Color::Magenta),
        )));
    }

    // Add hint to the content
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Esc] Menu  [Q] Quit",
        Style::default().fg(Color::Magenta),
    )));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
                .title(format!(" Extension to Graph H ({} vertices) ", n)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the mappings view
fn render_mappings(f: &mut Frame, app: &AppState, area: Rect) {
    let mappings = app.mappings.as_ref().unwrap();
    let current_idx = app.selected_mapping;
    let mapping = &mappings[current_idx];
    let n_g = mapping.len();
    let n_h = app.h.num_vertices();
    let viewport = &app.viewport_mappings;

    let mut lines = vec![];

    // Title with navigation
    lines.push(Line::from(vec![
        Span::styled("          ◄  ", Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("Mapping {} of {}  ", current_idx + 1, mappings.len()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("►", Style::default().fg(Color::Cyan)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Permutation Matrix - Each row shows where G vertex maps to in H",
        Style::default().fg(Color::Gray),
    )));
    lines.push(Line::from(""));

    // Calculate visible rows/cols based on terminal size
    let rows_visible = (area.height.saturating_sub(16) as usize).max(3);
    let cols_visible = ((area.width.saturating_sub(16)) / 4).max(5) as usize;

    let row_offset = viewport.row_offset.min(n_g.saturating_sub(1));
    let col_offset = viewport.col_offset.min(n_h.saturating_sub(1));

    let max_row = row_offset + rows_visible.min(n_g.saturating_sub(row_offset));
    let max_col = col_offset + cols_visible.min(n_h.saturating_sub(col_offset));

    // Header line with H vertex numbers
    let mut header = String::from("   H vertices: ");
    for col in col_offset..max_col {
        header.push_str(&format!("{:3} ", col));
    }
    if max_col < n_h {
        header.push_str(" ...");
    }
    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    // Separator line
    let mut separator = String::from("               ┌");
    for _ in col_offset..max_col {
        separator.push_str("────");
    }
    lines.push(Line::from(Span::styled(
        separator,
        Style::default().fg(Color::DarkGray),
    )));

    // Matrix rows
    for (idx, &h_vertex) in mapping.iter().enumerate().skip(row_offset).take(max_row - row_offset) {
        let g_vertex = row_offset + idx;
        let mut line_spans = vec![
            Span::styled(
                format!("   G[{:2}] → {:2}  ", g_vertex, h_vertex),
                Style::default().fg(Color::Green),
            ),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
        ];

        for col in col_offset..max_col {
            let symbol = if col == h_vertex {
                "◉" // Filled circle for the mapping
            } else {
                "·" // Middle dot for empty cells
            };

            let style = if col == h_vertex {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            line_spans.push(Span::styled(format!("  {}  ", symbol), style));
        }

        if max_col < n_h {
            line_spans.push(Span::styled("  ·", Style::default().fg(Color::DarkGray)));
        }

        lines.push(Line::from(line_spans));
    }

    // Show ellipsis if there are more rows
    if max_row < n_g {
        lines.push(Line::from(Span::styled(
            "               │  ...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Footer separator
    let mut footer_sep = String::from("               └");
    for _ in col_offset..max_col {
        footer_sep.push_str("────");
    }
    lines.push(Line::from(Span::styled(
        footer_sep,
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));

    // Legend
    lines.push(Line::from(vec![
        Span::styled("   ◉ = mapped    ", Style::default().fg(Color::Yellow)),
        Span::styled("· = not mapped", Style::default().fg(Color::DarkGray)),
    ]));

    // Navigation info
    lines.push(Line::from(""));
    if n_g > rows_visible || n_h > cols_visible {
        lines.push(Line::from(Span::styled(
            format!(
                "   Viewing rows {}-{}, cols {}-{} of {}x{}",
                row_offset,
                max_row.saturating_sub(1),
                col_offset,
                max_col.saturating_sub(1),
                n_g,
                n_h
            ),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "   [↑↓←→] Scroll  [PgUp/Dn] Jump rows  [[/]] Jump cols  [Home/End] First/Last",
            Style::default().fg(Color::Magenta),
        )));
    }

    lines.push(Line::from(Span::styled(
        "   [</,] Previous  [>/.]  Next mapping",
        Style::default().fg(Color::Magenta),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "   [Esc] Menu  [Q] Quit",
        Style::default().fg(Color::Magenta),
    )));

    let paragraph = Paragraph::new(lines).alignment(Alignment::Left).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(format!(" Permutation Matrix (k={}) ", app.k)),
    );

    f.render_widget(paragraph, area);
}

/// Render combined graphs view (G and H side by side)
fn render_graphs_combined(f: &mut Frame, app: &AppState, area: Rect) {
    // Split vertically: main content and hint bar at bottom
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    // Split horizontally for G and H
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[0]);

    render_graph_matrix_panel(
        f,
        &app.g,
        &app.viewport_g,
        "Graph G Adjacency Matrix",
        chunks[0],
    );
    render_graph_matrix_panel(
        f,
        &app.h,
        &app.viewport_h,
        "Graph H Adjacency Matrix",
        chunks[1],
    );

    // Navigation hint at bottom
    let hint = Paragraph::new("[Esc] Menu  [Q] Quit")
        .style(Style::default().fg(Color::Magenta))
        .alignment(Alignment::Center);
    f.render_widget(hint, main_chunks[1]);
}

/// Render a graph adjacency matrix panel (for combined view)
fn render_graph_matrix_panel(
    f: &mut Frame,
    graph: &Graph,
    viewport: &Viewport,
    title: &str,
    area: Rect,
) {
    let n = graph.num_vertices();

    // Calculate visible rows/cols based on panel size
    let rows_visible = (area.height.saturating_sub(6) as usize).max(3);
    let cols_visible = ((area.width.saturating_sub(6)) / 5).max(3) as usize;

    let row_offset = viewport.row_offset.min(n.saturating_sub(1));
    let col_offset = viewport.col_offset.min(n.saturating_sub(1));

    let max_row = row_offset + rows_visible.min(n.saturating_sub(row_offset));
    let max_col = col_offset + cols_visible.min(n.saturating_sub(col_offset));

    let mut lines = vec![];

    // Header line with column numbers
    let mut header = String::from("    ");
    for col in col_offset..max_col {
        header.push_str(&format!("{:4}", col));
    }
    if max_col < n {
        header.push_str(" ...");
    }
    lines.push(Line::from(Span::styled(
        header,
        Style::default().fg(Color::Cyan),
    )));

    // Matrix rows
    for row in row_offset..max_row {
        let mut line_spans = vec![Span::styled(
            format!("{:3}│", row),
            Style::default().fg(Color::Cyan),
        )];

        for col in col_offset..max_col {
            let value = graph.get_edge(row, col);
            let style = if value > 0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            line_spans.push(Span::styled(format!("{:4}", value), style));
        }

        if max_col < n {
            line_spans.push(Span::styled(" ...", Style::default().fg(Color::DarkGray)));
        }

        lines.push(Line::from(line_spans));
    }

    if max_row < n {
        lines.push(Line::from(Span::styled(
            "  ...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Navigation info
    if n > rows_visible || n > cols_visible {
        lines.push(Line::from(Span::styled(
            format!(
                "[{}-{}, {}-{}] of {}x{}",
                row_offset,
                max_row.saturating_sub(1),
                col_offset,
                max_col.saturating_sub(1),
                n,
                n
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(" {} ", title)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Main UI rendering
fn ui(f: &mut Frame, app: &AppState) {
    let size = f.area();

    match app.current_view {
        View::Calculating => render_calculating(f, app, size),
        View::Menu => render_menu(f, app, size),
        View::Graphs => render_graphs_combined(f, app, size),
        View::Extension => render_extension(f, app, size),
        View::Mappings => render_mappings(f, app, size),
    }
}

/// Run the exact algorithm in a background thread
fn run_exact_algorithm(g: Graph, h: Graph, k: usize, tx: Sender<ProgressMessage>) {
    let start_time = Instant::now();

    tx.send(ProgressMessage::Status(
        "Finding all possible mappings...".to_string(),
    ))
    .ok();
    let all_mappings = find_all_mappings(&g, &h);

    tx.send(ProgressMessage::Status(format!(
        "Found {} total mappings",
        all_mappings.len()
    )))
    .ok();

    if all_mappings.len() < k {
        tx.send(ProgressMessage::Error(format!(
            "Not enough mappings. Need {}, found {}",
            k,
            all_mappings.len()
        )))
        .ok();
        return;
    }

    let total_combinations = num_combinations(all_mappings.len(), k);
    tx.send(ProgressMessage::Status(format!(
        "Evaluating {} combinations...",
        total_combinations
    )))
    .ok();

    let best_cost: Mutex<usize> = Mutex::new(usize::MAX);
    let best_result: Mutex<Option<(EdgeMap, Vec<Mapping>)>> = Mutex::new(None);

    all_mappings
        .iter()
        .combinations(k)
        .par_bridge()
        .for_each(|combination| {
            let edge_map = calculate_edge_map(&g, &h, &combination);
            let total_cost = calculate_total_cost(&edge_map);

            {
                let current_best = best_cost.lock().unwrap();
                if total_cost >= *current_best {
                    return;
                }
            }

            {
                let mut cost_guard = best_cost.lock().unwrap();
                if total_cost < *cost_guard {
                    *cost_guard = total_cost;
                    drop(cost_guard);

                    let mappings = combination.iter().map(|&m| m.clone()).collect();
                    let mut result_guard = best_result.lock().unwrap();
                    *result_guard = Some((edge_map, mappings));
                }
            }
        });

    let final_cost = best_cost.into_inner().unwrap();
    let final_result = best_result.into_inner().unwrap();

    if let Some((edge_map, mappings)) = final_result {
        tx.send(ProgressMessage::Complete {
            cost: final_cost,
            edge_map,
            mappings,
            elapsed: start_time.elapsed(),
        })
        .ok();
    } else {
        tx.send(ProgressMessage::Error("No solution found".to_string()))
            .ok();
    }
}

/// Run the approximation algorithm in a background thread
fn run_approx_algorithm(g: Graph, h: Graph, k: usize, tx: Sender<ProgressMessage>) {
    let start_time = Instant::now();

    let mut h_prime = h.clone();
    let mut used_mappings = HashSet::new();
    let mut minimal_extension = EdgeMap::new();
    let mut all_mappings = Vec::new();

    tx.send(ProgressMessage::Status(format!(
        "Finding {} distinct mappings...",
        k
    )))
    .ok();

    for i in 1..=k {
        tx.send(ProgressMessage::MappingProgress {
            current: i,
            total: k,
        })
        .ok();

        match approximate_best_mapping(&g, &h_prime, &used_mappings, Some(&tx)) {
            Some((best_mapping, edges_to_add)) => {
                for ((x, y), weight) in edges_to_add.iter() {
                    let current = minimal_extension.get(&(*x, *y)).copied().unwrap_or(0);
                    if *weight > current {
                        minimal_extension.insert((*x, *y), *weight);
                    }
                }

                apply_extension(&mut h_prime, &g, &best_mapping);
                used_mappings.insert(best_mapping.clone());
                all_mappings.push(best_mapping);

                // Send success status
                tx.send(ProgressMessage::Status(format!(
                    "✓ Mapping {}/{} found",
                    i, k
                )))
                .ok();
            }
            None => {
                tx.send(ProgressMessage::Error(format!(
                    "Failed to find mapping {}/{}",
                    i, k
                )))
                .ok();
                return;
            }
        }
    }

    let total_cost: usize = minimal_extension.values().sum();

    tx.send(ProgressMessage::Complete {
        cost: total_cost,
        edge_map: minimal_extension,
        mappings: all_mappings,
        elapsed: start_time.elapsed(),
    })
    .ok();
}

/// Helper function for approximation algorithm
fn calculate_local_cost(
    u_i: usize,
    v_j: usize,
    g: &Graph,
    h_prime: &Graph,
    mapping: &HashMap<usize, usize>,
) -> usize {
    let mut cost = 0;

    for (&u_mapped, &v_mapped) in mapping.iter() {
        let g_edge = g.get_edge(u_i, u_mapped);
        if g_edge > 0 {
            let h_edge = h_prime.get_edge(v_j, v_mapped);
            cost += g_edge.saturating_sub(h_edge);
        }

        let g_edge_rev = g.get_edge(u_mapped, u_i);
        if g_edge_rev > 0 {
            let h_edge_rev = h_prime.get_edge(v_mapped, v_j);
            cost += g_edge_rev.saturating_sub(h_edge_rev);
        }
    }

    cost
}

fn apply_extension(h_prime: &mut Graph, g: &Graph, mapping: &Mapping) {
    for u in 0..g.num_vertices() {
        for v in 0..g.num_vertices() {
            let x = mapping[u];
            let y = mapping[v];
            let required = g.get_edge(u, v);

            if required > h_prime.get_edge(x, y) {
                h_prime.adj[x][y] = required;
            }
        }
    }
}

fn approximate_best_mapping(
    g: &Graph,
    h_prime: &Graph,
    used_mappings: &HashSet<Vec<usize>>,
    tx: Option<&Sender<ProgressMessage>>,
) -> Option<(Mapping, EdgeMap)> {
    let n_g = g.num_vertices();
    let n_h = h_prime.num_vertices();
    let t = n_g * n_h; // trials_multiplier = 1

    let mut min_global_cost = usize::MAX;
    let mut best_global_mapping: Option<Mapping> = None;
    let mut best_edges_to_add = EdgeMap::new();

    let mut rng = thread_rng();

    for trial in 0..t {
        // Send progress update every 500 trials
        if let Some(sender) = tx {
            if trial % 500 == 0 && trial > 0 {
                sender
                    .send(ProgressMessage::Status(format!("Trial {}/{}...", trial, t)))
                    .ok();
            }
        }
        let mut mapping_map: HashMap<usize, usize> = HashMap::new();
        let mut edges_to_add = EdgeMap::new();

        let g_vertices: Vec<usize> = (0..n_g).collect();
        let h_vertices: Vec<usize> = (0..n_h).collect();

        let u_start = g_vertices.choose(&mut rng).copied().unwrap();
        let v_start = h_vertices.choose(&mut rng).copied().unwrap();
        mapping_map.insert(u_start, v_start);

        let mut used_h_vertices = HashSet::new();
        used_h_vertices.insert(v_start);

        for u_i in 0..n_g {
            if mapping_map.contains_key(&u_i) {
                continue;
            }

            let mut min_local_cost = usize::MAX;
            let mut best_v_j = None;

            for v_j in 0..n_h {
                if used_h_vertices.contains(&v_j) {
                    continue;
                }

                let local_cost = calculate_local_cost(u_i, v_j, g, h_prime, &mapping_map);

                if local_cost < min_local_cost {
                    min_local_cost = local_cost;
                    best_v_j = Some(v_j);
                }
            }

            if let Some(v_j) = best_v_j {
                mapping_map.insert(u_i, v_j);
                used_h_vertices.insert(v_j);
            } else {
                break;
            }
        }

        if mapping_map.len() == n_g {
            let mapping_vec: Vec<usize> = (0..n_g).map(|i| mapping_map[&i]).collect();

            if used_mappings.contains(&mapping_vec) {
                continue;
            }

            let mut current_cost = 0;
            for u in 0..n_g {
                for v in 0..n_g {
                    let g_edge_count = g.get_edge(u, v);
                    if g_edge_count > 0 {
                        let x = mapping_vec[u];
                        let y = mapping_vec[v];
                        let h_edge_count = h_prime.get_edge(x, y);
                        let needed = g_edge_count.saturating_sub(h_edge_count);

                        if needed > 0 {
                            edges_to_add.insert((x, y), needed);
                            current_cost += needed;
                        }
                    }
                }
            }

            if current_cost < min_global_cost {
                min_global_cost = current_cost;
                best_global_mapping = Some(mapping_vec);
                best_edges_to_add = edges_to_add.clone();
            }
        }
    }

    best_global_mapping.map(|m| (m, best_edges_to_add))
}

/// Write results to a file
#[allow(clippy::too_many_arguments)]
fn write_results_to_file(
    path: &PathBuf,
    g: &Graph,
    h: &Graph,
    k: usize,
    algorithm: Algorithm,
    cost: usize,
    edge_map: &EdgeMap,
    mappings: &[Mapping],
    elapsed: Duration,
) -> io::Result<()> {
    let mut file = File::create(path)?;

    // Header
    writeln!(
        file,
        "============================================================"
    )?;
    writeln!(
        file,
        "Minimal k-Isomorphic Subgraph Extension - Solution Report"
    )?;
    writeln!(
        file,
        "============================================================"
    )?;
    writeln!(file)?;

    // Algorithm info
    writeln!(
        file,
        "Algorithm: {}",
        match algorithm {
            Algorithm::Exact => "Exact",
            Algorithm::Approx => "Approximation",
        }
    )?;
    writeln!(file, "k (required mappings): {}", k)?;
    writeln!(file, "Time: {}ms", elapsed.as_millis())?;
    writeln!(file, "Total Cost (edges added): {}", cost)?;
    writeln!(file)?;

    // Graph info
    writeln!(
        file,
        "------------------------------------------------------------"
    )?;
    writeln!(file, "Graph G (pattern): {} vertices", g.num_vertices())?;
    writeln!(
        file,
        "------------------------------------------------------------"
    )?;
    writeln!(file, "Adjacency Matrix:")?;
    for i in 0..g.num_vertices() {
        let row: Vec<String> = (0..g.num_vertices())
            .map(|j| format!("{:3}", g.get_edge(i, j)))
            .collect();
        writeln!(file, "  {}: [{}]", i, row.join(", "))?;
    }
    writeln!(file)?;

    // Graph H
    writeln!(
        file,
        "------------------------------------------------------------"
    )?;
    writeln!(file, "Graph H (host): {} vertices", h.num_vertices())?;
    writeln!(
        file,
        "------------------------------------------------------------"
    )?;
    writeln!(file, "Adjacency Matrix:")?;
    for i in 0..h.num_vertices() {
        let row: Vec<String> = (0..h.num_vertices())
            .map(|j| format!("{:3}", h.get_edge(i, j)))
            .collect();
        writeln!(file, "  {}: [{}]", i, row.join(", "))?;
    }
    writeln!(file)?;

    // Extended H matrix
    writeln!(file, "Extended H Matrix (original + added):")?;
    for i in 0..h.num_vertices() {
        let row: Vec<String> = (0..h.num_vertices())
            .map(|j| {
                let original = h.get_edge(i, j);
                let added = edge_map.get(&(i, j)).copied().unwrap_or(0);
                if added > 0 {
                    format!("{:3}+{}", original, added)
                } else {
                    format!("{:5}", original)
                }
            })
            .collect();
        writeln!(file, "  {}: [{}]", i, row.join(", "))?;
    }
    writeln!(file)?;

    // Mappings
    writeln!(
        file,
        "------------------------------------------------------------"
    )?;
    writeln!(file, "Mappings (Permutation Matrix Format)")?;
    writeln!(
        file,
        "------------------------------------------------------------"
    )?;
    for (idx, mapping) in mappings.iter().enumerate() {
        writeln!(file, "\nMapping {} of {}:", idx + 1, mappings.len())?;
        writeln!(
            file,
            "Permutation Matrix - G vertices (rows) to H vertices (columns)"
        )?;
        writeln!(file)?;

        // Header with H vertex numbers
        write!(file, "  H vertices: ")?;
        for h in 0..h.num_vertices() {
            write!(file, "{:2} ", h)?;
        }
        writeln!(file)?;

        // Top border
        write!(file, "              ┌")?;
        for _ in 0..h.num_vertices() {
            write!(file, "───")?;
        }
        writeln!(file)?;

        // Matrix rows
        for (g_vertex, &h_vertex) in mapping.iter().enumerate() {
            write!(file, "  G[{:2}] → {:2}  │", g_vertex, h_vertex)?;
            for h in 0..h.num_vertices() {
                if h == h_vertex {
                    write!(file, "◉ ")?; // Filled circle for mapping
                } else {
                    write!(file, "· ")?; // Middle dot for empty
                }
            }
            writeln!(file)?;
        }

        // Bottom border
        write!(file, "              └")?;
        for _ in 0..h.num_vertices() {
            write!(file, "───")?;
        }
        writeln!(file)?;

        writeln!(file)?;
        writeln!(file, "  ◉ = mapped    · = not mapped")?;

        // Also include simple list for reference
        writeln!(file)?;
        writeln!(file, "  Mapping list: G[vertex] → H[vertex]")?;
        for (g_vertex, &h_vertex) in mapping.iter().enumerate() {
            write!(file, "    G[{}]→H[{}]", g_vertex, h_vertex)?;
            if (g_vertex + 1) % 8 == 0 || g_vertex == mapping.len() - 1 {
                writeln!(file)?;
            } else {
                write!(file, "  ")?;
            }
        }
        writeln!(file)?;
    }

    writeln!(
        file,
        "\n============================================================"
    )?;
    writeln!(file, "End of Report")?;
    writeln!(
        file,
        "============================================================"
    )?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Parse input graphs
    let (g, h) = match parse_input_file(&args.input) {
        Ok(graphs) => graphs,
        Err(e) => {
            eprintln!("Error parsing input file: {}", e);
            std::process::exit(1);
        }
    };

    // Validate k
    if args.k == 0 {
        eprintln!("Error: k must be at least 1");
        std::process::exit(1);
    }

    // Determine if we should save to file
    // Save to file if: either graph has >15 vertices OR --output-file was specified
    let output_file =
        if g.num_vertices() > 15 || h.num_vertices() > 15 || args.output_file.is_some() {
            Some(args.output_file.unwrap_or_else(|| {
                let algo_name = match args.algorithm {
                    Algorithm::Exact => "exact",
                    Algorithm::Approx => "approx",
                };
                PathBuf::from(format!("solution_{}.txt", algo_name))
            }))
        } else {
            None
        };

    // Always use interactive TUI
    // Create channel for progress updates
    let (tx, rx) = channel();

    // Spawn algorithm thread
    let g_clone = g.clone();
    let h_clone = h.clone();
    let k = args.k;
    let algorithm = args.algorithm;

    thread::spawn(move || match algorithm {
        Algorithm::Exact => run_exact_algorithm(g_clone, h_clone, k, tx),
        Algorithm::Approx => run_approx_algorithm(g_clone, h_clone, k, tx),
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = AppState::new(args.algorithm, g, h, args.k, output_file, rx);

    // Main loop
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events, not release (fixes Windows double-trigger)
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        if app.current_view != View::Calculating {
                            break;
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }
                    _ => app.handle_key(key.code),
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.update()?;
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
