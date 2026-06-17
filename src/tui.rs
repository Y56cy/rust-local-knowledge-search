use crate::app::{AppMode, AppState};
use crate::highlighter::clean_for_terminal;
use crate::utils::bytes_to_human;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use std::io;

pub fn run(mut app: AppState) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_loop(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                break;
            }

            let should_quit = match app.mode {
                AppMode::Search => handle_search_key(app, key)?,
                AppMode::Preview => {
                    handle_preview_key(app, key);
                    false
                }
                AppMode::Help | AppMode::History | AppMode::Bookmarks | AppMode::Stats => {
                    handle_panel_key(app, key)?
                }
            };
            if should_quit {
                break;
            }
        }
    }
    Ok(())
}

fn handle_search_key(app: &mut AppState, key: KeyEvent) -> Result<bool> {
    if app.command_mode {
        return handle_command_key(app, key);
    }

    match key.code {
        KeyCode::Esc => return Ok(true),
        KeyCode::Enter => app.search()?,
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Up => app.previous(),
        KeyCode::Down => app.next(),
        KeyCode::Tab => app.open_preview()?,
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.enter_command_mode();
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.add_selected_bookmark()?;
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.input.push(c);
        }
        _ => {}
    }
    Ok(false)
}

fn handle_command_key(app: &mut AppState, key: KeyEvent) -> Result<bool> {
    app.command_mode = false;
    match key.code {
        KeyCode::Esc => app.cancel_command_mode(),
        KeyCode::Char('p') => app.open_preview()?,
        KeyCode::Char('b') => app.add_selected_bookmark()?,
        KeyCode::Char('h') => app.switch_mode(AppMode::History),
        KeyCode::Char('m') => app.switch_mode(AppMode::Bookmarks),
        KeyCode::Char('s') => app.switch_mode(AppMode::Stats),
        KeyCode::Char('u') => {
            app.incremental_update()?;
        }
        KeyCode::Char('?') => app.switch_mode(AppMode::Help),
        KeyCode::Char('q') => return Ok(true),
        _ => app.message = "Unknown command. Press Ctrl+O for commands.".into(),
    }
    Ok(false)
}

fn handle_preview_key(app: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Tab | KeyCode::Char('q') => app.switch_mode(AppMode::Search),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_preview_up(1),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_preview_down(1),
        KeyCode::PageUp => app.scroll_preview_up(10),
        KeyCode::PageDown => app.scroll_preview_down(10),
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.switch_mode(AppMode::Help);
        }
        _ => {}
    }
}

fn handle_panel_key(app: &mut AppState, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc | KeyCode::Tab | KeyCode::Char('q') => app.switch_mode(AppMode::Search),
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.switch_mode(AppMode::Help);
        }
        KeyCode::Char('c') if app.mode == AppMode::History => app.clear_history()?,
        KeyCode::Up if app.mode == AppMode::Bookmarks => app.previous_bookmark(),
        KeyCode::Down if app.mode == AppMode::Bookmarks => app.next_bookmark(),
        KeyCode::Enter if app.mode == AppMode::Bookmarks => app.open_selected_bookmark()?,
        KeyCode::Delete if app.mode == AppMode::Bookmarks => app.remove_selected_bookmark()?,
        KeyCode::Char('d')
            if app.mode == AppMode::Bookmarks && key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.remove_selected_bookmark()?;
        }
        _ => {}
    }
    Ok(false)
}

fn render(frame: &mut ratatui::Frame, app: &AppState) {
    let area = frame.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(4),
        ])
        .split(area);

    let input_title = if app.command_mode {
        format!("Command | matches: {} | p/b/h/m/s/u/?/q", app.total_matches)
    } else {
        format!(
            "Query | matches: {} | mode: {:?}",
            app.total_matches, app.mode
        )
    };
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().title(input_title).borders(Borders::ALL));
    frame.render_widget(input, chunks[0]);

    match app.mode {
        AppMode::Search => render_results(frame, app, chunks[1]),
        AppMode::Preview => render_preview(frame, app, chunks[1]),
        AppMode::Help => render_help(frame, chunks[1]),
        AppMode::History => render_history(frame, app, chunks[1]),
        AppMode::Bookmarks => render_bookmarks(frame, app, chunks[1]),
        AppMode::Stats => render_stats(frame, app, chunks[1]),
    }

    let footer = Paragraph::new(footer_text(app))
        .wrap(Wrap { trim: true })
        .block(Block::default().title("Status").borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

fn footer_text(app: &AppState) -> String {
    let keys = match app.mode {
        AppMode::Search => {
            "Enter search | Up/Down select | Tab preview | Ctrl+B bookmark | Ctrl+O commands | Esc quit"
        }
        AppMode::Preview => "Up/Down scroll | PageUp/PageDown | Tab/Esc return",
        AppMode::History => "c clear history | Ctrl+O help | Tab/Esc return",
        AppMode::Bookmarks => {
            "Up/Down select | Enter preview | Delete/Ctrl+D remove | Ctrl+O help | Tab/Esc return"
        }
        AppMode::Stats | AppMode::Help => "Ctrl+O help | Tab/Esc return",
    };
    format!("{}  |  {}", app.message, keys)
}

fn render_results(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|result| {
            let snippet = clean_for_terminal(&result.snippet, 180);
            ListItem::new(vec![
                Line::from(Span::raw(format!(
                    "[{:.2}] {} | matches:{} | .{} | {}",
                    result.score,
                    result.title,
                    result.match_count,
                    result.extension,
                    bytes_to_human(result.bytes)
                ))),
                Line::from(Span::raw(format!("  {}", result.path.display()))),
                Line::from(Span::raw(format!("  {}", snippet))),
            ])
        })
        .collect();

    let mut state = ListState::default();
    if !app.results.is_empty() {
        state.select(Some(app.selected));
    }
    let title = format!(
        "Results: {} / {} | Up/Down select | Tab preview | Ctrl+B bookmark",
        app.results.len(),
        app.total_matches
    );
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_preview(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let scroll = app.preview_scroll.min(u16::MAX as usize) as u16;
    let preview = Paragraph::new(app.preview_text.as_str())
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(
            Block::default()
                .title("Preview | Up/Down scroll | PageUp/PageDown | Tab/Esc return")
                .borders(Borders::ALL),
        );
    frame.render_widget(preview, area);
}

fn render_help(frame: &mut ratatui::Frame, area: Rect) {
    let text = vec![
        Line::from("Local Knowledge Search Help"),
        Line::from(""),
        Line::from("Typing: input query text"),
        Line::from("Enter: search"),
        Line::from("Up/Down: select result"),
        Line::from("Tab: preview selected result"),
        Line::from("Ctrl+B: bookmark selected result"),
        Line::from("Ctrl+O then p: preview selected result"),
        Line::from("Ctrl+O then b: bookmark selected result"),
        Line::from("Ctrl+O then h: search history"),
        Line::from("Ctrl+O then m: bookmarks"),
        Line::from("Ctrl+O then s: index statistics"),
        Line::from("Ctrl+O then u: incremental update"),
        Line::from("Ctrl+O then q: quit"),
        Line::from("Delete or Ctrl+D in bookmarks: remove bookmark"),
        Line::from("Esc: return or quit"),
    ];
    let help = Paragraph::new(text).block(Block::default().title("Help").borders(Borders::ALL));
    frame.render_widget(help, area);
}

fn render_history(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .history
        .iter()
        .rev()
        .take(30)
        .map(|history| {
            ListItem::new(Line::from(format!(
                "{} | {} matches | {}",
                history.query, history.result_count, history.searched_at
            )))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .title("Search History | c clear | Tab/Esc return")
            .borders(Borders::ALL),
    );
    frame.render_widget(list, area);
}

fn render_bookmarks(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let items: Vec<ListItem> = app
        .bookmarks
        .iter()
        .map(|bookmark| {
            ListItem::new(vec![
                Line::from(bookmark.title.clone()),
                Line::from(format!("  {}", bookmark.path.display())),
            ])
        })
        .collect();

    let mut state = ListState::default();
    if !app.bookmarks.is_empty() {
        state.select(Some(app.bookmark_selected));
    }
    let list = List::new(items)
        .block(
            Block::default()
                .title("Bookmarks | Up/Down select | Enter preview | Delete remove")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_stats(frame: &mut ratatui::Frame, app: &AppState, area: Rect) {
    let mut lines = vec![
        Line::from(format!("Documents: {}", app.stats.documents)),
        Line::from(format!(
            "Total size: {}",
            bytes_to_human(app.stats.total_bytes)
        )),
        Line::from(""),
        Line::from("By extension:"),
    ];
    for stat in &app.stats.extensions {
        lines.push(Line::from(format!(
            ".{}: {} files, {}",
            stat.extension,
            stat.count,
            bytes_to_human(stat.bytes)
        )));
    }
    let stats = Paragraph::new(lines).block(
        Block::default()
            .title("Index Statistics | Tab/Esc return")
            .borders(Borders::ALL),
    );
    frame.render_widget(stats, area);
}
