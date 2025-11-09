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
    pub fn as_text(&self) -> Text {
        match self {
            BoardPost::Message { username, content } => {
                // Text::from(format!("{username}: {content}"))
                Text::from(vec![
                    Line::from(username.clone()).bold(),
                    Line::from(": ".to_string()),
                    Line::from(content.clone()),
                ])
            }
            BoardPost::Connected(user) => Text::from(format!("{user} connected").italic()),
            BoardPost::Disconnected(user) => Text::from(format!("{user} disconnected").italic()),
        }
    }

    pub(crate) fn as_line(&self) -> Line<'_> {
        match self {
            BoardPost::Message { username, content } => {
                Line::from(vec![username.clone().bold(), ": ".into(), content.into()])
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
