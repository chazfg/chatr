use chatr::{Content, Username};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::{Line, Text},
    widgets::Widget,
};

#[derive(Debug)]
pub enum BoardPost {
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
    pub fn as_text(&self) -> Text {
        match self {
            BoardPost::Message { username, content } => {
                Text::from(format!("{username}: {content}"))
            }
            BoardPost::Connected(user) => Text::from(format!("{user} connected").italic()),
            BoardPost::Disconnected(user) => Text::from(format!("{user} disconnected").italic()),
        }
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
