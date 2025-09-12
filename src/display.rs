use crate::{
    PENDING_REQUESTS_LIMIT, SystemEvent, SystemState, SystemStats, request::Request,
    server::ServerState,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame, Terminal, backend,
    layout::{self, Constraint, Layout, Rect},
    prelude::CrosstermBackend,
    style::{self, Style},
    text,
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use std::{
    collections::VecDeque,
    io,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::Receiver;

struct AtomicRect {
    x: AtomicUsize,
    y: AtomicUsize,
    width: AtomicUsize,
    height: AtomicUsize,
}

impl AtomicRect {
    const fn new() -> Self {
        Self {
            x: AtomicUsize::new(0),
            y: AtomicUsize::new(0),
            width: AtomicUsize::new(0),
            height: AtomicUsize::new(0),
        }
    }

    fn update_from(&self, rect: Rect) {
        self.x.store(rect.x as usize, Ordering::SeqCst);
        self.y.store(rect.y as usize, Ordering::SeqCst);
        self.width.store(rect.width as usize, Ordering::SeqCst);
        self.height.store(rect.height as usize, Ordering::SeqCst);
    }

    fn contains(&self, x: u16, y: u16) -> bool {
        let self_x = self.x.load(Ordering::SeqCst) as u16;
        let self_y = self.y.load(Ordering::SeqCst) as u16;
        let self_width = self.width.load(Ordering::SeqCst) as u16;
        let self_height = self.height.load(Ordering::SeqCst) as u16;

        x >= self_x && x < self_x + self_width && y >= self_y && y < self_y + self_height
    }
}

static SELECTED_LOG: AtomicUsize = AtomicUsize::new(0);

static SERVER_AREAS: [AtomicRect; 3] = [AtomicRect::new(), AtomicRect::new(), AtomicRect::new()];
static SERVER_SCROLL: [AtomicUsize; 3] = [
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
];

pub fn run_ui(mut ui_rx: Receiver<SystemEvent>) -> io::Result<()> {
    let mut terminal = init_terminal()?;

    let mut system_state = SystemState {
        pending_requests: VecDeque::new(),
        servers: [
            ServerState::new(1),
            ServerState::new(2),
            ServerState::new(3),
        ],
        logs: Vec::with_capacity(100),
        stats: SystemStats {
            total_requests: 0,
            processed_requests: 0,
            avg_wait_time: 0.0,
        },
    };

    let mut last_frame = Instant::now();
    let frame_rate = Duration::from_millis(33); // 30 FPS

    loop {
        let elapsed = last_frame.elapsed();
        if elapsed < frame_rate {
            thread::sleep(frame_rate - elapsed);
        }
        last_frame = Instant::now();

        while let Ok(event) = ui_rx.try_recv() {
            update_system_state(&mut system_state, event);
        }

        terminal.draw(|frame| {
            render_system_ui(frame, &system_state);
        })?;

        if handle_events()? {
            break;
        }
    }

    restore_terminal(&mut terminal).ok();
    Ok(())
}

fn init_terminal() -> io::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    let stdout = io::stdout();
    let backend = backend::CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )
    .ok();

    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> io::Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn update_system_state(state: &mut SystemState, event: SystemEvent) {
    match event {
        SystemEvent::RequestCreated(request) => {
            state.pending_requests.push_back(request);
            state.stats.total_requests += 1;
            add_log(&mut state.logs, format!("Request #{} created", request.id));
        }
        SystemEvent::RequestAssigned { server_id, request } => {
            state.pending_requests.retain(|r| r.id != request.id);

            let server_idx = (server_id - 1) as usize;
            if server_idx < state.servers.len() {
                state.servers[server_idx].add_request(request);
                add_log(
                    &mut state.logs,
                    format!("Request #{} assigned to Server {}", request.id, server_id),
                );
            }
        }
        SystemEvent::RequestProcessStarted {
            request_id,
            server_id,
        } => {
            add_log(
                &mut state.logs,
                format!(
                    "Server {} started processing Request #{}",
                    server_id, request_id
                ),
            );
        }
        SystemEvent::RequestProcessed {
            request_id,
            server_id,
        } => {
            let server_idx = (server_id - 1) as usize;
            if server_idx < state.servers.len() {
                let server = &mut state.servers[server_idx];
                server.remove_request();

                state.stats.processed_requests += 1;

                add_log(
                    &mut state.logs,
                    format!("Server {} processed request #{}", server_id, request_id),
                );
            }
        }
        SystemEvent::ErrorEncountered(error_msg) => {
            add_log(&mut state.logs, format!("Error: {error_msg}"));
        }
    }
}

fn add_log(logs: &mut Vec<String>, message: String) {
    if logs.len() >= logs.capacity() {
        logs.remove(0);
    };

    logs.push(format!(
        "[{}] {}",
        chrono::Local::now().format("%H:%M:%S"),
        message
    ));
}

fn render_system_ui(frame: &mut Frame, state: &SystemState) {
    let main_layout = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
        .areas(frame.area());
    let [processing_area, info_area] = main_layout;

    let processing_layout =
        Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)])
            .areas(processing_area);
    let [requests_area, servers_area] = processing_layout;

    let info_layout =
        Layout::vertical([Constraint::Percentage(30), Constraint::Percentage(70)]).areas(info_area);
    let [stats_area, logs_area] = info_layout;

    render_requests(frame, requests_area, &state.pending_requests);
    render_servers(frame, servers_area, &state.servers);
    render_stats(frame, stats_area, &state.stats);
    render_logs(frame, logs_area, &state.logs);
}

fn render_requests(frame: &mut Frame, area: Rect, requests: &VecDeque<Request>) {
    let style = if requests.len() >= PENDING_REQUESTS_LIMIT as usize {
        Style::default().fg(style::Color::Red)
    } else {
        Style::default()
    };
    let block = Block::bordered().title("Pending Requests").style(style);
    let inner_area = block.inner(area);

    frame.render_widget(block, area);

    if !requests.is_empty() {
        let req_width = 21;
        let req_height = 4;

        let requests_per_row = (inner_area.width as usize / req_width).max(1);
        let rows_available = (inner_area.width as usize / req_height).max(1);

        let mut constraints_h = Vec::new();
        let mut constraints_v = Vec::new();

        for _ in 0..requests_per_row {
            constraints_h.push(Constraint::Length(req_width as u16));
        }
        for _ in 0..requests_per_row {
            constraints_v.push(Constraint::Length(req_height as u16));
        }

        let mut request_idx = 0;
        for row in 0..rows_available {
            if request_idx >= requests.len() {
                break;
            }

            for col in 0..requests_per_row {
                if request_idx >= requests.len() {
                    break;
                }

                let request = &requests[request_idx];

                let cell_x = (col * req_width) as u16 + inner_area.x;
                let cell_y = (row * req_height) as u16 + inner_area.y;
                let cell_area = Rect::new(cell_x, cell_y, req_width as u16, req_height as u16);

                let req_block = Block::bordered().style(first_req_style(request_idx));
                frame.render_widget(req_block.clone(), cell_area);

                let inner_req_area = req_block.inner(cell_area);
                let req_text = Paragraph::new(format!("{}\n(#{})", request.get_name(), request.id))
                    .alignment(layout::Alignment::Center);

                frame.render_widget(req_text, inner_req_area);

                request_idx += 1;
            }
        }
    }
}

fn render_servers(frame: &mut Frame, area: Rect, servers: &[ServerState; 3]) {
    let servers_layout = Layout::horizontal([Constraint::Fill(1); 3]).split(area);

    for i in 0..3 {
        SERVER_AREAS[i].update_from(servers_layout[i]);
    }

    for (idx, server) in servers.iter().enumerate() {
        let style = if server.queue.len() >= server.queue.capacity() {
            Style::default().fg(style::Color::Red)
        } else {
            Style::default()
        };

        let server_block = Block::bordered()
            .title(format!(
                "Server {} (Load {}ms)",
                server.id, server.total_workload
            ))
            .style(style);

        let inner_area = server_block.inner(servers_layout[idx]);

        frame.render_widget(server_block, servers_layout[idx]);

        if !server.queue.is_empty() {
            let visible_height = inner_area.height as usize / 3; // Each item is 3 rows tall
            let visible_items = visible_height.max(1);

            let scroll_pos = SERVER_SCROLL[idx]
                .load(Ordering::SeqCst)
                .min(server.queue.len().saturating_sub(visible_items));

            let visible_requests = server.queue.iter().skip(scroll_pos).take(visible_items);

            let req_layout =
                Layout::vertical(vec![Constraint::Length(3); visible_items]).split(inner_area);

            for (req_idx, request) in visible_requests.enumerate() {
                let req_text = Paragraph::new(text::Line::raw(format!(
                    "{} (#{}) - {}ms",
                    request.get_name(),
                    request.id,
                    request.get_time()
                )))
                .alignment(layout::Alignment::Center)
                .block(Block::bordered().style(first_req_style(req_idx)));

                frame.render_widget(req_text, req_layout[req_idx]);
            }
        }
    }
}

fn first_req_style(idx: usize) -> Style {
    if idx == 0 {
        Style::default().fg(style::Color::Green)
    } else {
        Style::default()
    }
}

fn render_stats(frame: &mut Frame, area: Rect, stats: &SystemStats) {
    let block = Block::bordered().title("Statistics");
    let inner_area = block.inner(area);

    frame.render_widget(block, area);

    let stats_text = text::Text::from(vec![
        text::Line::from(format!("Total Requests: {}", stats.total_requests)),
        text::Line::from(format!("Processed: {}", stats.processed_requests)),
        text::Line::from(format!("Average Wait: {:.1}ms", stats.avg_wait_time)),
    ]);

    let stats_widget = Paragraph::new(stats_text);
    frame.render_widget(stats_widget, inner_area);
}

fn render_logs(frame: &mut Frame, area: Rect, logs: &Vec<String>) {
    let block = Block::bordered().title("Event Log");
    let inner_area = block.inner(area);

    frame.render_widget(block, area);

    if !logs.is_empty() {
        let items: Vec<ListItem> = logs
            .iter()
            .map(|log| ListItem::new(text::Line::from(log.clone())))
            .rev()
            .collect();

        let max_scroll = items.len().saturating_sub(1);
        let current_log = SELECTED_LOG.load(Ordering::SeqCst);
        SELECTED_LOG.store(current_log.min(max_scroll), Ordering::SeqCst);

        let logs_list = List::new(items)
            .block(Block::default())
            .highlight_style(Style::default().add_modifier(style::Modifier::REVERSED));

        let mut state = ListState::default();
        state.select(Some(SELECTED_LOG.load(Ordering::SeqCst)));

        frame.render_stateful_widget(logs_list, inner_area, &mut state);
    }
}

fn handle_events() -> io::Result<bool> {
    if event::poll(Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') => return Ok(true),
                _ => {}
            },
            Event::Mouse(mouse) => {
                let position = (mouse.column, mouse.row);

                match mouse.kind {
                    crossterm::event::MouseEventKind::ScrollUp
                    | crossterm::event::MouseEventKind::ScrollDown => {
                        let is_scrolling_up =
                            matches!(mouse.kind, crossterm::event::MouseEventKind::ScrollUp);

                        let mut hit_server = None;
                        {
                            for idx in 0..3 {
                                if SERVER_AREAS[idx].contains(position.0, position.1) {
                                    hit_server = Some(idx);
                                    break;
                                }
                            }

                            if let Some(idx) = hit_server {
                                let current = SERVER_SCROLL[idx].load(Ordering::SeqCst);
                                if is_scrolling_up {
                                    SERVER_SCROLL[idx]
                                        .store(current.saturating_add(1), Ordering::SeqCst);
                                } else {
                                    SERVER_SCROLL[idx]
                                        .store(current.saturating_sub(1), Ordering::SeqCst);
                                }
                            } else {
                                let current = SELECTED_LOG.load(Ordering::SeqCst);
                                if is_scrolling_up {
                                    SELECTED_LOG.store(current.saturating_add(1), Ordering::SeqCst);
                                } else {
                                    SELECTED_LOG.store(current.saturating_sub(1), Ordering::SeqCst);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(false)
}
