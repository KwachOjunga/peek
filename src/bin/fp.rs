use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use std::{
     io::stdout, path::PathBuf
};

#[derive(Parser, Debug)]
#[command(author, version, about = "Peek at file contents with smooth scrolling")]
struct Args {
    /// File to view
    filename: PathBuf,

    /// Fixed number of lines to display (default: terminal height - 2)
    #[arg(short, long, default_value = "30")]
    lines: Option<usize>,

    /// Start displaying with this line at the top (1-based)
    #[arg(short = 's', long)]
    start_line: Option<usize>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let lines = std::fs::read_to_string(&args.filename)
        .with_context(|| format!("Failed to read file: {}", args.filename.display()))?
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    if lines.is_empty() {
        eprintln!("File is empty.");
        return Ok(());
    }

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let res = run_app(&mut terminal, lines, args.lines, args.start_line, args.filename);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    res
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    file_lines: Vec<String>,
    fixed_height: Option<usize>,
    start_line: Option<usize>,
    file_name: PathBuf,
) -> Result<()> {
    let total_lines = file_lines.len();
    let mut scroll = start_line.unwrap_or(1).saturating_sub(1); // 0-based

    loop {
        terminal.draw(|frame| {
            let size = frame.area();

            // Determine visible height (leave 2 lines for border + status)
            let available_height = size.height.saturating_sub(2) as usize;
            let visible_lines = fixed_height.unwrap_or(available_height).min(available_height);

            // Clamp scroll
            if total_lines <= visible_lines {
                scroll = 0;
            } else {
                scroll = scroll.min(total_lines - visible_lines);
            }

            let content_lines: Vec<Line<'_>> = file_lines
                .iter()
                .skip(scroll)
                .take(visible_lines)
                .map(|s| Line::from(s.as_str()))
                .collect();

            let paragraph = Paragraph::new(content_lines)
                .block(Block::default().borders(Borders::ALL).title(format!(
                    " {} ",
                    file_name.display()
                )))
                .scroll((0, 0)); // No horizontal scroll for now

            let status = format!(
                "Line {}-{} of {} | ↑↓/j k: line | PgUp/PgDn: page | g/G: top/bottom | q: quit",
                scroll + 1,
                (scroll + visible_lines).min(total_lines),
                total_lines
            );

            let status_line = Line::from(status).style(Style::default().fg(Color::Yellow));

            // Layout: content + status
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(size);

            frame.render_widget(paragraph, chunks[0]);
            frame.render_widget(Paragraph::new(status_line), chunks[1]);

            // Vertical scrollbar
            let mut scrollbar_state = ScrollbarState::new(total_lines.saturating_sub(visible_lines))
                .position(scroll);
            frame.render_stateful_widget(
                Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                chunks[0],
                &mut scrollbar_state,
            );
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('j') | KeyCode::Down => {
                    if scroll < total_lines.saturating_sub(1) {
                        scroll += 1;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    scroll = scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    let visible = fixed_height.unwrap_or(terminal.size()?.height as usize - 2);
                    scroll = scroll.saturating_add(visible).min(total_lines.saturating_sub(1));
                }
                KeyCode::PageUp => {
                    let visible = fixed_height.unwrap_or(terminal.size()?.height as usize - 2);
                    scroll = scroll.saturating_sub(visible);
                }
                KeyCode::Char('g') => scroll = 0,
                KeyCode::Char('G') => {
                    let visible = fixed_height.unwrap_or(terminal.size()?.height as usize - 2);
                    scroll = total_lines.saturating_sub(visible);
                }
                _ => {}
            }
        }
    }
}
