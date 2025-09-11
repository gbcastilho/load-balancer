use crate::{SystemEvent, SystemState, SystemStats, request::Request, server::ServerState};
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
    io, thread,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::Receiver;

static mut SELECTED_LOG: usize = 0;

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
    let block = Block::bordered().title("Pending Requests");
    let inner_area = block.inner(area);

    frame.render_widget(block, area);

    if !requests.is_empty() {
        let req_width = 21;
        let requests_per_row = (inner_area.width as usize / req_width).max(1);

        let req_layout = Layout::horizontal(vec![
            Constraint::Length(20);
            requests.len().min(requests_per_row)
        ])
        .split(inner_area);

        for (idx, request) in requests.iter().enumerate().take(requests_per_row) {
            let req_block = Block::bordered();
            frame.render_widget(req_block.clone(), req_layout[idx]);

            let inner_req_area = req_block.inner(req_layout[idx]);
            let req_text = Paragraph::new(format!("{}\n(#{})", request.get_name(), request.id))
                .alignment(layout::Alignment::Center);

            frame.render_widget(req_text, inner_req_area);
        }
    }
}

fn render_servers(frame: &mut Frame, area: Rect, servers: &[ServerState; 3]) {
    let servers_layout = Layout::horizontal([Constraint::Fill(1); 3]).split(area);

    for (idx, server) in servers.iter().enumerate() {
        let server_block = Block::bordered().title(format!(
            "Server {} (Load {}ms)",
            server.id, server.total_workload
        ));

        let inner_area = server_block.inner(servers_layout[idx]);

        frame.render_widget(server_block, servers_layout[idx]);

        if !server.queue.is_empty() {
            let req_layout =
                Layout::vertical(vec![Constraint::Length(3); server.queue.len()]).split(inner_area);

            for (req_idx, request) in server.queue.iter().enumerate() {
                let style = if req_idx == 0 && server.is_processing {
                    Style::default().fg(style::Color::Green)
                } else {
                    Style::default()
                };

                let req_text = Paragraph::new(text::Line::styled(
                    format!(
                        "{} (#{}) - {}ms",
                        request.get_name(),
                        request.id,
                        request.get_time()
                    ),
                    style,
                ))
                .alignment(layout::Alignment::Center)
                .block(Block::bordered());

                frame.render_widget(req_text, req_layout[req_idx]);
            }
        }
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
        unsafe {
            SELECTED_LOG = SELECTED_LOG.min(max_scroll);
        }

        let logs_list = List::new(items)
            .block(Block::default())
            .highlight_style(Style::default().add_modifier(style::Modifier::REVERSED));

        let mut state = ListState::default();
        unsafe {
            state.select(Some(SELECTED_LOG));
        }

        frame.render_stateful_widget(logs_list, inner_area, &mut state);
    }
}

fn handle_events() -> io::Result<bool> {
    static mut SCROLL_POSITION: usize = 0;

    if event::poll(Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') => return Ok(true),
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                crossterm::event::MouseEventKind::ScrollUp => unsafe {
                    SCROLL_POSITION = SCROLL_POSITION.saturating_add(1);
                    SELECTED_LOG = SCROLL_POSITION;
                },
                crossterm::event::MouseEventKind::ScrollDown => unsafe {
                    SCROLL_POSITION = SCROLL_POSITION.saturating_sub(1);
                    SELECTED_LOG = SCROLL_POSITION;
                },
                _ => {}
            },
            _ => {}
        }
    }

    Ok(false)
}
