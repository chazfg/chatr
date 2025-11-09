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

/// Message schema
#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub enum ChatrMessage {
    /// LoginRequest sent when a user is trying to connect to the chatroom
    LoginRequest { username: String },
    /// Received when user allowed to join
    LoginAccepted,
    /// Received when user rejected, reason given
    LoginRejected { reason: String },
    /// User sends message without username to save space
    SentMessage { content: Content },
    /// SentMessage becomes recieved message when the chatroom gets the message and then tags it
    /// with the username
    ReceivedMessage {
        username: Username,
        content: Content,
    },
    /// Event emitted on user connection
    UserConnected { username: Username },
    /// Event emitted on user disconnection
    UserDisconnected { username: Username },
    /// Received/Sent when some end of the connection is done
    Disconnect,
}
