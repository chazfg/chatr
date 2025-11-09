use std::{io, vec};

use chatr::{ChatrMessage, client::ClientConnection};
use crossterm::event::{self, Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect, Spacing},
    style::{Color, Stylize},
    text::{Line, Text},
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget, Wrap,
    },
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::sync::CancellationToken;

use crate::chatr_widgets::{
    board_post::BoardPost,
    text_box::{TextBox, TitledTextBox},
};
pub mod chatr_widgets;

#[tokio::main]
async fn main() -> io::Result<()> {
    color_eyre::install().unwrap();
    let mut terminal = ratatui::init();
    let result = App::default().run(&mut terminal).await;
    ratatui::restore();
    result
}

#[derive(Debug, Default)]
struct App {
    message_board: MessageBoard,
    buffer: TextBox,
    exit: bool,
}

#[derive(Debug, Default)]
struct MessageBoard {
    messages: Vec<BoardPost>,
}

impl MessageBoard {
    pub fn user_disconnected(&mut self, username: String) {
        self.messages.push(BoardPost::Disconnected(username));
    }
    pub fn user_connected(&mut self, username: String) {
        self.messages.push(BoardPost::Connected(username));
    }
    pub fn post_message(&mut self, username: String, content: String) {
        self.messages.push(BoardPost::Message { username, content });
    }
}

impl Widget for &MessageBoard {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let block = Block::default()
            .title_top(" Chatr ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain);
        let inner = block.inner(area);
        let msgs = self
            .messages
            .iter()
            .map(|m| m.as_line())
            .collect::<Vec<Line>>();
        let para = Paragraph::new(msgs).block(block).wrap(Wrap { trim: false });
        let content_height = para.line_count(inner.width);
        let view_height = inner.height as usize;
        let max_scroll = content_height.saturating_sub(view_height);
        let para = para.scroll((max_scroll as u16, 0));
        para.render(area, buf);
        let mut sb_state = ScrollbarState::new(content_height).position(max_scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        scrollbar.render(area, buf, &mut sb_state);
    }
}

#[derive(Debug)]
struct Button {
    text: String,
    selected: bool,
}
impl Button {
    pub fn unselect(&mut self) {
        self.selected = false;
    }
    pub fn select(&mut self) {
        self.selected = true;
    }
}

impl Widget for &Button {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        if self.selected {
            Text::from(self.text.clone())
                .bg(Color::White)
                .fg(Color::Black)
                .render(area, buf);
        } else {
            Text::from(self.text.clone())
                .fg(Color::White)
                .bg(Color::Black)
                .render(area, buf);
        }
    }
}

#[derive(Debug)]
struct LoginFlow {
    username: TitledTextBox,
    host: TitledTextBox,
    submit_button: Button,
    exit: bool,
    selected_item: u8,
}

impl Default for LoginFlow {
    fn default() -> Self {
        let mut user_text = TextBox::default();
        user_text.select();
        Self {
            username: TitledTextBox::new(user_text, "username", true),
            host: TitledTextBox::title("host"),
            exit: Default::default(),
            selected_item: Default::default(),
            submit_button: Button {
                text: "Submit".to_string(),
                selected: false,
            },
        }
    }
}

impl Widget for &LoginFlow {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let row_constraints = vec![
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ];
        let horizontal = Layout::vertical(row_constraints).spacing(Spacing::Space(0));
        let rows = horizontal.split(area);
        self.username.render(rows[0], buf);
        self.host.render(rows[1], buf);
        self.submit_button.render(rows[2], buf);
    }
}
impl LoginFlow {
    fn exit(&mut self) {
        self.exit = true;
    }
    pub async fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
        event_stream: &mut EventStream,
    ) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events(event_stream).await?;
        }
        Ok(())
    }
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
    fn on_enter(&mut self) {
        if self.selected_item == 2 {
            self.exit()
        } else {
            self.select_down()
        }
    }
    async fn handle_events(&mut self, event_stream: &mut EventStream) -> io::Result<()> {
        match event_stream.next().await {
            Some(Ok(Event::Key(key_event))) if key_event.kind == KeyEventKind::Press => {
                if key_event.modifiers != KeyModifiers::CONTROL {
                    match key_event.code {
                        KeyCode::Up => self.select_up(),
                        KeyCode::Down => self.select_down(),
                        KeyCode::Enter => self.on_enter(),
                        a => {
                            if self.selected_item == 0 {
                                self.username.handle_key_code(a);
                            } else {
                                self.host.handle_key_code(a)
                            }
                        }
                    }
                } else if key_event.code == KeyCode::Char('q') {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "user quit"));
                }
            }
            None => todo!(),
            x => todo!("{x:?}"),
        }
        Ok(())
    }

    fn set_selection(&mut self) {
        match self.selected_item {
            0 => {
                self.username.select();
                self.host.unselect();
                self.submit_button.unselect();
            }
            1 => {
                self.username.unselect();
                self.host.select();
                self.submit_button.unselect();
            }
            2 => {
                self.username.unselect();
                self.host.unselect();
                self.submit_button.select();
            }
            _ => panic!(),
        }
    }

    fn select_down(&mut self) {
        self.selected_item += 1;
        if self.selected_item == 3 {
            self.selected_item = 0;
        }
        self.set_selection();
    }

    fn select_up(&mut self) {
        if self.selected_item == 0 {
            self.selected_item = 2;
        } else {
            self.selected_item -= 1;
        }
        self.set_selection();
    }

    fn verify(&mut self) -> io::Result<(String, String)> {
        let user = self.username.take_buffer();
        let host = self.host.take_buffer();
        Ok((user, host))
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut event_stream = event::EventStream::new();
        let mut lf = LoginFlow::default();
        lf.run(terminal, &mut event_stream).await.unwrap();
        let (username, host) = lf.verify().unwrap();
        let mut client_conn = ClientConnection::new(&host).await.unwrap();
        client_conn.login(username).await.unwrap();
        let ct = CancellationToken::new();
        let (s1, mut r1) = tokio::sync::mpsc::channel(1024);
        let (mut s2, r2) = tokio::sync::mpsc::channel(1024);
        client_conn.run(s1, r2, ct);
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events(&mut event_stream, &mut r1, &mut s2)
                .await?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
    fn exit(&mut self) {
        self.exit = true;
    }
    async fn send_message(&mut self, send_message: &mut Sender<ChatrMessage>) -> io::Result<()> {
        let msg = self.buffer.take_buffer();
        if msg.is_empty() {
            Ok(())
        } else {
            send_message
                .send(ChatrMessage::SentMessage { content: msg })
                .await
                .unwrap();
            Ok(())
        }
    }

    async fn handle_events(
        &mut self,
        event_stream: &mut EventStream,
        new_messages: &mut Receiver<ChatrMessage>,
        send_message: &mut Sender<ChatrMessage>,
    ) -> io::Result<()> {
        tokio::select! {
            event = event_stream.next() => match event {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Some(Ok(Event::Key(key_event))) if key_event.kind == KeyEventKind::Press => {
                if key_event.modifiers != KeyModifiers::CONTROL {
                        if key_event.code != KeyCode::Enter {
                            self.buffer.handle_key_event(key_event);
                        } else if !self.buffer.is_empty() {
                        self.send_message(send_message).await?;
                        }
                    } else if key_event.code == KeyCode::Char('q') {
                        self.exit()
                    }

            }
            _ => {}
            },
            new_msg = new_messages.recv() => {
                match new_msg {
                    Some(ChatrMessage::ReceivedMessage { username, content }) => self.message_board.post_message(username, content),
                    Some(ChatrMessage::UserConnected{username}) => self.message_board.user_connected(username),
                    Some(ChatrMessage::UserDisconnected{username}) => self.message_board.user_disconnected(username),
                    None => todo!(),
                    x => todo!("{x:?}"),
                }
            }

        }
        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let row_constraints = vec![Constraint::Fill(1), Constraint::Length(2)];
        let horizontal = Layout::vertical(row_constraints).spacing(Spacing::Space(0));
        let rows = horizontal.split(area);
        self.message_board.render(rows[0], buf);
        self.buffer.render(rows[1], buf);
    }
}
