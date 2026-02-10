#![allow(unused)]

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
// use ratatui_themes::{Color as ThemesColor, Theme, ThemeName};
use std::{io::stdout, path::PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about = "Peek at file contents with smooth scrolling")]
struct Args {
    /// File to view
    filename: PathBuf,

    /// Fixed number of lines to display (default: 70)
    #[arg(short, long, default_value = "70")]
    lines: Option<usize>,

    /// Start displaying with this line at the top (1-based)
    #[arg(short = 's', long)]
    start_line: Option<usize>,
}

// Dracula palette (official hex → RGB)
const DRACULA_BG: Color = Color::Rgb(40, 42, 54); // #282A36
const DRACULA_FG: Color = Color::Rgb(248, 248, 242); // #F8F8F2
const DRACULA_COMMENT: Color = Color::Rgb(98, 114, 164); // #6272A4
const DRACULA_PURPLE: Color = Color::Rgb(189, 147, 249); // #BD93F9 (keywords)
const DRACULA_CYAN: Color = Color::Rgb(139, 233, 253); // #8BE9FD (functions, types)
const DRACULA_GREEN: Color = Color::Rgb(80, 250, 123); // #50FA7B (strings)
const DRACULA_ORANGE: Color = Color::Rgb(255, 184, 108); // #FFB86C (numbers/constants)
const DRACULA_RED: Color = Color::Rgb(255, 85, 85); // #FF5555 (errors/warnings)
const DRACULA_PINK: Color = Color::Rgb(255, 121, 198); // #FF79C6 (special)
const DRACULA_YELLOW: Color = Color::Rgb(241, 250, 140); // #F1FA8C (warnings/numbers alt)
const DRACULA_CURRENT_LINE: Color = Color::Rgb(68, 71, 90); // #44475A (subtle highlight)

fn highlight_line(line: &str) -> Line<'_> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // --- Comments ---
        if (c == '/' && i + 1 < chars.len() && chars[i + 1] == '/')
            || c == '#'
            || (c == '/' && i + 1 < chars.len() && chars[i + 1] == '*')
        {
            let comment = &line[i..];
            spans.push(Span::styled(
                comment.to_string(),
                Style::default().fg(DRACULA_COMMENT).italic(),
            ));
            break;
        }

        if c.is_whitespace() {
            spans.push(Span::raw(c.to_string()));
            i += 1;
            continue;
        }

        if c.is_alphabetic() || c == '_' || c.is_ascii_digit() {
            let start = i;
            i += 1;

            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }

            let word: String = chars[start..i].iter().collect();

            let mut style = Style::default().fg(DRACULA_FG);

            if is_keyword(&word) {
                style = style.fg(DRACULA_PURPLE).bold();
            }

            if is_type(&word) {
                style = style.fg(DRACULA_CYAN);
            }

            if word.parse::<f64>().is_ok() {
                style = style.fg(DRACULA_ORANGE);
            }

            spans.push(Span::styled(word, style));
            continue;
        }

        // --- Symbols / punctuation ---
        spans.push(Span::styled(c.to_string(), Style::default().fg(DRACULA_FG)));
        i += 1;
    }

    if spans.is_empty() {
        Line::from(line)
    } else {
        Line::from(spans)
    }
}

// fn highlight_line(line: &str) -> Line<'_> {
//     let mut spans = Vec::new();
//     let mut current = String::new();
//     let chars: Vec<char> = line.chars().collect();
//     let mut i = 0;

//     while i < chars.len() {
//         let c = chars[i];

//         // Skip whitespace quickly
//         if c.is_whitespace() {
//             current.push(c);
//             i += 1;
//             continue;
//         }

//         // Start collecting word
//         if current.is_empty() && c.is_alphabetic() || c == '_' {
//             current.push(c);
//             i += 1;
//             while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
//                 current.push(chars[i]);
//                 i += 1;
//             }

//             // Classify word
//             let style = if is_keyword(&current) {
//                 Style::default().fg(DRACULA_PURPLE).bold()
//             } else if is_type(&current) {
//                 Style::default().fg(DRACULA_CYAN)
//             } else if is_string_delim(c) {
//                 // handle strings roughly
//                 Style::default().fg(DRACULA_GREEN)
//             } else if current.parse::<f64>().is_ok() {
//                 Style::default().fg(DRACULA_ORANGE)
//             } else {
//                 Style::default().fg(DRACULA_FG)
//             };

//             spans.push(Span::styled(current, style));
//             current = String::new();
//             continue;
//         }

//         // Comments (// or # or /* */ rough detection)
//         if (c == '/' && i + 1 < chars.len() && chars[i + 1] == '/')
//             || c == '#'
//             || (c == '/' && i + 1 < chars.len() && chars[i + 1] == '*')
//         {
//             // Rest of line is comment
//             let comment = line[i..].to_string();
//             spans.push(Span::styled(
//                 comment,
//                 Style::default().fg(DRACULA_COMMENT).italic(),
//             ));
//             break;
//         }

//         // Punctuation/symbols
//         current.push(c);
//         i += 1;
//     }

//     if !current.is_empty() {
//         spans.push(Span::styled(current, Style::default().fg(DRACULA_FG)));
//     }

//     if spans.is_empty() {
//         Line::from(line)
//     } else {
//         Line::from(spans)
//     }
// }

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "let"
            | "mut"
            | "const"
            | "struct"
            | "enum"
            | "impl"
            | "trait"
            | "pub"
            | "use"
            | "if"
            | "else"
            | "match"
            | "for"
            | "while"
            | "loop"
            | "return"
            | "break"
            | "continue"
            | "true"
            | "false"
            | "None"
            | "Some"
            | "Ok"
            | "Err"
    )
}

fn is_type(word: &str) -> bool {
    matches!(
        word,
        "String"
            | "Vec"
            | "Option"
            | "Result"
            | "i32"
            | "u64"
            | "f64"
            | "bool"
            | "char"
            | "usize"
            | "PathBuf"
            | "Result"
            | "Option"
    )
}

fn is_string_delim(c: char) -> bool {
    c == '"' || c == '\''
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
    // let theme = Theme::new(ThemeName::Dracula);
    // let palette = theme.palette();
    // let mut app = App::new(file_lines, fixed_height, scroll, file_name);

    loop {
        terminal.draw(|frame| {
            let size = frame.area();

            // Determine visible height (leave 2 lines for border + status)
            let available_height = size.height.saturating_sub(2) as usize;
            let visible_lines = fixed_height
                .unwrap_or(available_height)
                .min(available_height);

            // Clamp scroll
            if total_lines <= visible_lines {
                scroll = 0;
            } else {
                scroll = scroll.min(total_lines - visible_lines);
            }

            // let content_lines: Vec<Line<'_>> = file_lines
            //     .iter()
            //     .skip(scroll)
            //     .take(visible_lines)
            //     .map(|s| Line::from(s.as_str()))
            //     .collect();

            let content_lines: Vec<Line<'_>> = file_lines
                .iter()
                .skip(scroll)
                .take(visible_lines)
                .map(|s| highlight_line(s))
                .collect();

            let paragraph = Paragraph::new(content_lines)
                .style(
                    Style::default().fg(Color::Rgb(248, 248, 242)), // .bg(Color::Rgb(40, 42, 54)),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {} ", file_name.display())),
                )
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
            let mut scrollbar_state =
                ScrollbarState::new(total_lines.saturating_sub(visible_lines)).position(scroll);
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
                    scroll = scroll
                        .saturating_add(visible)
                        .min(total_lines.saturating_sub(1));
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

    let res = run_app(
        &mut terminal,
        lines,
        args.lines,
        args.start_line,
        args.filename,
    );

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    res
}
