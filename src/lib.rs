use borsh::{BorshDeserialize, BorshSerialize};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
pub mod chatroom;
pub mod client;

pub type Username = String;
pub type Content = String;
pub type SenderToClient = Sender<ChatrMessage>;
pub type SenderToServer = Sender<(Username, ChatrMessage)>;
pub type ReceiverFromClient = Receiver<(Username, ChatrMessage)>;
pub type ReceiverFromServer = Receiver<ChatrMessage>;

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub enum ChatrMessage {
    LoginRequest {
        username: String,
    },
    LoginAccepted,
    LoginRejected {
        reason: String,
    },
    SentMessage {
        content: Content,
    },
    ReceivedMessage {
        username: Username,
        content: Content,
    },
    UserConnected {
        username: Username,
    },
    UserDisconnected {
        username: Username,
    },
    Disconnect,
}
