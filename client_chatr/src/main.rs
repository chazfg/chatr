use std::{io, vec};

use chatr::{ChatrMessage, Content, Username, client::ClientConnection};
use crossterm::event::{self, Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Margin, Rect, Spacing},
    style::{Color, Style, Styled, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{
        Block, List, ListDirection, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::sync::CancellationToken;

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

enum KeyEventSideEffect {
    Exit,
    SendMessage(String),
}

#[derive(Debug)]
enum BoardPost {
    Message {
        username: Username,
        content: Content,
    },
    Connected(Username),
    Disconnected(Username),
}
impl BoardPost {
    fn list_item(&self) -> String {
        match self {
            BoardPost::Message { username, content } => todo!(),
            BoardPost::Connected(_) => todo!(),
            BoardPost::Disconnected(_) => todo!(),
        }
        // format!("{}: {}", self.username, self.content)
    }
    fn as_line(&self) -> Line {
        match self {
            BoardPost::Message { username, content } => {
                Line::from(format!("{username}: {content}"))
            }
            BoardPost::Connected(user) => Line::from(format!("{user} connected").italic()),
            BoardPost::Disconnected(user) => Line::from(format!("{user} disconnected").italic()),
        }
    }
}

impl Widget for &BoardPost {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        match self {
            BoardPost::Message { username, content } => {
                Line::from(format!("{username}: {content}")).render(area, buf);
            }
            BoardPost::Connected(user) => {
                Line::from(format!("{user} connected").italic()).render(area, buf);
            }
            BoardPost::Disconnected(user) => {
                Line::from(format!("{user} disconnected").italic()).render(area, buf);
            }
        }
    }
}

#[derive(Debug, Default)]
struct MessageBoard {
    messages: Vec<BoardPost>,
    scrollbar: ScrollbarState,
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
        let items: Vec<Line> = self.messages.iter().map(|m| m.as_line()).collect();

        let mut scrollbar_state = ScrollbarState::new(items.len())
            .viewport_content_length(15)
            .position(0);
        let pg = Paragraph::new(items).scroll((0 as u16, 0));
        // Note we render the paragraph
        pg.render(area, buf);
        // and the scrollbar, those are separate widgets
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        scrollbar.render(
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            buf,
            &mut scrollbar_state,
        );
        // let list = List::new(self.messages.iter().map(|m| m.list_item()))
        //     .block(Block::bordered().title(" Chatr "))
        //     .style(Style::new().white())
        //     .highlight_style(Style::new().italic())
        //     .highlight_symbol(">>")
        //     .repeat_highlight_symbol(true)
        //     .direction(ListDirection::TopToBottom);
        // list.render(area, buf);
    }
}

#[derive(Debug, Default)]
struct TextBox {
    buffer: String,
    cursor: Cursor,
    selected: bool,
}
#[derive(Debug, Default)]
struct Cursor {
    position: u16,
    inverted: bool,
}

impl Cursor {
    pub fn unselect(&mut self) {
        self.inverted = false;
    }
    pub fn select(&mut self) {
        self.inverted = true;
    }
    pub fn forward(&mut self) {
        self.position += 1;
    }
    pub fn backward(&mut self) {
        if self.position != 0 {
            self.position -= 1;
        }
    }
    pub fn position(&self) -> usize {
        self.position as usize
    }
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

impl TextBox {
    pub fn unselect(&mut self) {
        self.selected = false;
        self.cursor.unselect();
    }
    pub fn select(&mut self) {
        self.selected = true;
        self.cursor.select();
    }
    pub fn is_empty(&mut self) -> bool {
        self.buffer.is_empty()
    }
    pub fn take_buffer(&mut self) -> String {
        self.cursor.reset();
        std::mem::take(&mut self.buffer)
    }
    fn handle_key_code(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Char(c) => {
                if self.cursor.position() == self.buffer.len() {
                    self.buffer.push(c);
                    self.cursor.forward();
                } else {
                    self.buffer.insert(self.cursor.position(), c);
                    self.cursor.forward();
                }
            }
            KeyCode::Backspace => {
                if !self.buffer.is_empty() {
                    if self.cursor.position() == self.buffer.len() {
                        if self.buffer.pop().is_some() {
                            self.cursor.backward()
                        }
                    } else if self.cursor.position() != 0 {
                        self.buffer.remove(self.cursor.position() - 1);
                        self.cursor.backward();
                    }
                }
            }
            KeyCode::Delete => {
                if !self.buffer.is_empty() && self.cursor.position() != self.buffer.len() {
                    self.buffer.remove(self.cursor.position());
                }
            }
            KeyCode::Left => self.cursor.backward(),
            KeyCode::Right => {
                if self.cursor.position() < self.buffer.len() {
                    self.cursor.forward()
                }
            }
            _ => {}
        }
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.handle_key_code(key_event.code);
    }
}

#[derive(Debug)]
struct LoginFlow {
    username: TitledTextBox,
    host: TitledTextBox,
    exit: bool,
    selected_item: u8,
}

impl Default for LoginFlow {
    fn default() -> Self {
        let mut user_text = TextBox::default();
        user_text.select();
        Self {
            username: TitledTextBox {
                text_box: user_text,
                title: "username".to_string(),
                selected: true,
            },
            host: TitledTextBox {
                text_box: TextBox::default(),
                title: "host".to_string(),
                selected: false,
            },
            exit: Default::default(),
            selected_item: Default::default(),
        }
    }
}

#[derive(Debug, Default)]
struct TitledTextBox {
    text_box: TextBox,
    title: String,
    selected: bool,
}
impl TitledTextBox {
    pub fn take_buffer(&mut self) -> String {
        self.text_box.take_buffer()
    }
    pub fn unselect(&mut self) {
        self.selected = false;
        self.text_box.unselect();
    }
    pub fn select(&mut self) {
        self.selected = true;
        self.text_box.select();
    }
    pub fn handle_key_code(&mut self, key_code: KeyCode) {
        self.text_box.handle_key_code(key_code);
    }
}
impl Widget for &TitledTextBox {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let block = if self.selected {
            Block::new()
                .title_top(self.title.clone())
                .bg(Color::White)
                .fg(Color::Black)
        } else {
            Block::new().title_top(self.title.clone())
        };
        let inner = block.inner(area);
        block.render(area, buf);
        self.text_box.render(inner, buf);
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
            Constraint::Fill(1),
        ];
        let horizontal = Layout::vertical(row_constraints).spacing(Spacing::Space(0));
        let rows = horizontal.split(area);
        self.username.render(rows[0], buf);
        self.host.render(rows[1], buf);
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
    async fn handle_events(&mut self, event_stream: &mut EventStream) -> io::Result<()> {
        match event_stream.next().await {
            Some(Ok(Event::Key(key_event))) if key_event.kind == KeyEventKind::Press => {
                if key_event.modifiers != KeyModifiers::CONTROL {
                    match key_event.code {
                        KeyCode::Up => self.select_up(),
                        KeyCode::Down => self.select_down(),
                        KeyCode::Enter => self.exit(),
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
        if self.selected_item == 0 {
            self.username.select();
            self.host.unselect();
        } else {
            self.username.unselect();
            self.host.select();
        }
    }

    fn select_up(&mut self) {
        self.selected_item = (self.selected_item + 1) % 2;
        self.set_selection();
    }

    fn select_down(&mut self) {
        if self.selected_item == 0 {
            self.selected_item = 1;
        } else {
            self.selected_item = 0;
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

        // match event::read()? {
        // };
        Ok(())
    }
}

impl Widget for &TextBox {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        if self.selected {
            Line::from(self.buffer.clone())
                .bg(Color::White)
                .fg(Color::Black)
                .render(area, buf);
            self.cursor.render(area, buf);
        } else {
            Line::from(self.buffer.clone()).render(area, buf);
        }
    }
}
impl Widget for &Cursor {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        if self.inverted {
            buf[(area.x + self.position, area.y)]
                .set_fg(Color::White)
                .set_bg(Color::Black);
        } else {
            buf[(area.x + self.position, area.y)]
                .set_bg(Color::White)
                .set_fg(Color::Black);
        }
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
