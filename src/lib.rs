use std::io;

use borsh::{BorshDeserialize, BorshSerialize};
use bytes::BytesMut;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
pub mod chatroom;

pub type Username = String;
pub type Content = String;
pub type SenderToClient = Sender<ChatrMessage>;
pub type SenderToServer = Sender<(Username, ChatrMessage)>;
pub type ReceiverFromClient = Receiver<(Username, ChatrMessage)>;
pub type ReceiverFromServer = Receiver<ChatrMessage>;

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub enum ChatrMessage {
    LoginRequest { username: String },
    LoginAccepted,
    LoginRejected { reason: String },
    SentMessage { content: String },
    ReceivedMessage { username: String, content: String },
    Disconnect,
}

pub struct ClientConnection {
    pub stream: TcpStream,
    pub buf: bytes::BytesMut,
}

impl ClientConnection {
    pub async fn new(host: &str) -> io::Result<Self> {
        TcpStream::connect(host).await.map(|stream| Self {
            stream,
            buf: BytesMut::zeroed(1024),
        })
    }
    pub async fn login(&mut self, username: Username) -> io::Result<()> {
        self.stream
            .write_all(
                borsh::to_vec(&ChatrMessage::LoginRequest { username })
                    .unwrap()
                    .as_slice(),
            )
            .await?;
        self.stream.flush().await.unwrap();
        match self.stream.read(&mut self.buf).await? {
            0 => Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "0 bytes on login",
            )),
            n => match ChatrMessage::deserialize(&mut &self.buf[..n]) {
                Ok(login_msg) => match login_msg {
                    ChatrMessage::LoginAccepted => Ok(()),
                    ChatrMessage::LoginRejected { reason } => {
                        Err(io::Error::new(io::ErrorKind::ConnectionRefused, reason))
                    }
                    ChatrMessage::Disconnect => Err(io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        "recv disconnect",
                    )),
                    _ => Err(io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        "unreasonable msg",
                    )),
                },
                Err(e) => Err(io::Error::new(io::ErrorKind::InvalidInput, e)),
            },
        }
    }
    #[instrument(level = "debug", skip_all)]
    pub fn run(
        self,
        from_server_to_client: Sender<ChatrMessage>,
        mut to_server_from_client: Receiver<ChatrMessage>,
        _ct: CancellationToken,
    ) {
        let ClientConnection { stream, mut buf } = self;
        let (mut stream_reader, mut stream_writer) = stream.into_split();
        tokio::spawn(async move {
            while let Some(msg_to_send) = to_server_from_client.recv().await {
                tracing::trace!("{msg_to_send:?}");
                stream_writer
                    .write_all(&borsh::to_vec(&msg_to_send).unwrap())
                    .await
                    .unwrap();
                stream_writer.flush().await.unwrap();
            }
        });
        tokio::spawn(async move {
            loop {
                match stream_reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        from_server_to_client
                            .send(ChatrMessage::deserialize(&mut &buf[..n]).unwrap())
                            .await
                            .unwrap();
                    }
                    Err(_) => todo!(),
                }
            }
        });
        tracing::info!("run called");
    }
}
