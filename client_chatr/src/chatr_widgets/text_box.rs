use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Stylize},
    text::Line,
    widgets::{Block, Widget},
};

/// Handles input/events for typing
#[derive(Debug, Default)]
pub struct TextBox {
    buffer: String,
    cursor: Cursor,
    selected: bool,
}
/// Little square to show where text will get placed/deleted from a TextBox
#[derive(Debug, Default)]
pub struct Cursor {
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
    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.handle_key_code(key_event.code);
    }
}

/// TextBox, but with a title
#[derive(Debug, Default)]
pub struct TitledTextBox {
    text_box: TextBox,
    title: String,
    selected: bool,
}
impl TitledTextBox {
    pub fn new(text_box: TextBox, title: &str, selected: bool) -> Self {
        Self {
            text_box,
            title: title.to_string(),
            selected,
        }
    }
    pub fn title(title: &str) -> Self {
        Self {
            title: title.to_string(),
            ..Default::default()
        }
    }
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
