use std::{collections::VecDeque, io, sync::Arc, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{self, Constraint, Layout, Rect},
    widgets::{self, Block, Paragraph},
};
use tokio::sync::RwLock;

use crate::entities::Request;

pub fn draw(req_queue: Arc<RwLock<VecDeque<Request>>>) -> io::Result<()> {
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| {
            let vertical = Layout::vertical([Constraint::Min(1), Constraint::Min(1)]);
            let [req_area, servers_area] = vertical.areas(frame.area());
            render_req_widget(frame, req_area, &req_queue);
            frame.render_widget(Block::bordered().title("Servers"), servers_area);
        })?;
        if handle_events()? {
            break Ok(());
        }
    }
}

fn render_req_widget(frame: &mut Frame, area: Rect, req_queue: &Arc<RwLock<VecDeque<Request>>>) {
    let widget_block = Block::bordered().title("Arriving Requests");
    let inner_area = widget_block.inner(area);

    frame.render_widget(widget_block, area);

    let req_queue_guard = req_queue.blocking_read();

    if !req_queue_guard.is_empty() {
        let vert_layout = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]);
        let [req_line, _] = vert_layout.areas(inner_area);

        let req_layout =
            Layout::horizontal(vec![Constraint::Max(21); req_queue_guard.len()]).split(req_line);

        for (idx, request) in req_queue_guard.iter().enumerate() {
            let req_block = Block::bordered();

            frame.render_widget(req_block.clone(), req_layout[idx]);

            let req_area = req_block.inner(req_layout[idx]);
            let req_text = Paragraph::new(format!("{}", request.get_name()))
                .alignment(layout::Alignment::Center)
                .block(Block::bordered());

            frame.render_widget(req_text, req_area);
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
