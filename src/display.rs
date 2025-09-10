use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use once_cell::sync::Lazy;
use ratatui::{
    Frame,
    layout::{self, Constraint, Layout, Rect},
    text,
    widgets::{Block, Paragraph},
};
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::entities::{Request, Server};

static DEBUG_LOGS: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::with_capacity(100)));

pub fn draw(
    req_queue: Arc<RwLock<VecDeque<Request>>>,
    servers: Arc<[Arc<RwLock<Server>>; 3]>,
) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut last_frame = Instant::now();
    let frame_rate = Duration::from_millis(33); // 30 FPS

    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;

    loop {
        let elapsed = last_frame.elapsed();
        if elapsed < frame_rate {
            thread::sleep(frame_rate - elapsed);
        }
        last_frame = Instant::now();

        let req_guard = req_queue.blocking_read();
        let servers_guards: Vec<RwLockReadGuard<'_, Server>> =
            servers.iter().map(|s| s.blocking_read()).collect();

        terminal.draw(|frame| {
            let main_layout =
                Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .areas(frame.area());

            let [main_area, log_area] = main_layout;

            let vertical = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]);
            let [req_area, servers_area] = vertical.areas(main_area);

            render_req_widget(frame, req_area, &req_guard);
            render_servers_widget(frame, servers_area, &servers_guards);
            render_log(frame, log_area);
        })?;
        if handle_events()? {
            break;
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;

    Ok(())
}

fn render_req_widget(
    frame: &mut Frame,
    area: Rect,
    req_queue_guard: &RwLockReadGuard<'_, VecDeque<Request>>,
) {
    let widget_block = Block::bordered().title("Arriving Requests");
    let inner_area = widget_block.inner(area);

    frame.render_widget(widget_block, area);

    if !req_queue_guard.is_empty() {
        let vert_layout = Layout::vertical([Constraint::Length(4), Constraint::Min(0)]);
        let [req_line, _] = vert_layout.areas(inner_area);

        let req_layout =
            Layout::horizontal(vec![Constraint::Max(21); req_queue_guard.len()]).split(req_line);

        for (idx, request) in req_queue_guard.iter().enumerate() {
            let req_block = Block::bordered();

            frame.render_widget(req_block.clone(), req_layout[idx]);

            let req_area = req_block.inner(req_layout[idx]);
            let req_text = Paragraph::new(format!("{}\n(#{})", request.get_name(), request.id))
                .alignment(layout::Alignment::Center);
            frame.render_widget(req_text, req_area);
        }
    }
}

fn render_servers_widget(
    frame: &mut Frame,
    area: Rect,
    servers_guards: &[RwLockReadGuard<'_, Server>],
) {
    let svr_areas = Layout::horizontal([Constraint::Fill(1); 3]).split(area);

    for (idx, server_guard) in servers_guards.iter().enumerate() {
        let server_area = svr_areas[idx];
        let server_block = Block::bordered().title(format!("Server {}", server_guard.id));

        let inner_area = server_block.inner(server_area);

        frame.render_widget(server_block, server_area);

        let req_areas = Layout::vertical(vec![Constraint::Length(3); server_guard.queue.len()])
            .split(inner_area);

        for (idx, request) in server_guard.queue.iter().enumerate() {
            let req_text = Paragraph::new(format!("{} (#{})", request.get_name(), request.id))
                .alignment(layout::Alignment::Center)
                .block(Block::bordered());

            frame.render_widget(req_text, req_areas[idx]);
        }
    }
}

fn handle_events() -> io::Result<bool> {
    if event::poll(Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') => return Ok(true),
                _ => {}
            },
            _ => {}
        }
    }

    Ok(false)
}

pub fn log_debug(message: impl Into<String>) {
    let mut logs = DEBUG_LOGS.lock().unwrap();
    logs.push(format!(
        "[{}] {}",
        chrono::Local::now().format("%H:%M:%S"),
        message.into()
    ));
    if logs.len() > 99 {
        logs.remove(0);
    }
}

pub fn render_log(frame: &mut Frame, area: Rect) {
    let logs = DEBUG_LOGS.lock().unwrap();

    let log_block = Block::bordered()
        .title("Debug Logs")
        .title_alignment(layout::Alignment::Center);

    let inner_area = log_block.inner(area);
    frame.render_widget(log_block, area);

    let visible_logs = logs.len().min(inner_area.height as usize).max(1);
    let start_idx = logs.len().saturating_sub(visible_logs);

    let text: text::Text = logs
        .iter()
        .skip(start_idx)
        .map(|log| text::Line::from(text::Span::raw(log)))
        .rev()
        .collect();

    let log_widget = Paragraph::new(text).scroll((0, 0));

    frame.render_widget(log_widget, inner_area);
}
