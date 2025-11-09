use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;

use borsh::BorshDeserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, trace};

use crate::{ChatrMessage, Content, ReceiverFromServer, SenderToClient, SenderToServer, Username};

pub enum AdminMsg {
    AddClient(Username, SenderToClient),
    RemoveClient(Username),
    DispatchMsg(Username, Content),
}
#[derive(Default, Debug)]
pub struct Chatroom {
    clients: HashMap<Username, (CancellationToken, SenderToClient)>,
}
pub async fn send_to_clients(
    clients: &mut HashMap<Username, (CancellationToken, SenderToClient)>,
    msg: ChatrMessage,
) {
    for (_, (_, stc)) in clients.iter_mut() {
        stc.send(msg.clone()).await.unwrap()
    }
}
impl Chatroom {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }
    pub fn run(self, mut rx: mpsc::Receiver<AdminMsg>) {
        let Self { mut clients } = self;
        let ct = CancellationToken::new();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    AdminMsg::AddClient(username, sender) => {
                        clients.insert(username.clone(), (ct.clone(), sender));
                        send_to_clients(&mut clients, ChatrMessage::UserConnected { username })
                            .await;
                    }
                    AdminMsg::RemoveClient(username) => {
                        clients.remove(&username);
                        send_to_clients(&mut clients, ChatrMessage::UserDisconnected { username })
                            .await;
                    }
                    AdminMsg::DispatchMsg(username, content) => {
                        let msg = ChatrMessage::ReceivedMessage { username, content };
                        send_to_clients(&mut clients, msg).await;
                    }
                }
            }
        });
    }
    pub async fn add_new_client(
        &mut self,
        client: AuthenticatedClient,
        sender_to_server: SenderToServer,
    ) {
        let (sender_to_client, receiver_from_server): (SenderToClient, ReceiverFromServer) =
            mpsc::channel(1024);
        let cancel_token = CancellationToken::new();
        self.clients.insert(
            client.username.clone(),
            (cancel_token.clone(), sender_to_client),
        );
        client.run(sender_to_server, receiver_from_server, cancel_token);
    }
    pub async fn remove_client(
        &mut self,
        user: String,
    ) -> Option<(CancellationToken, SenderToClient)> {
        self.clients.remove(&user)
    }
    pub async fn dispatch_msg(&mut self, username: String, content: String) {
        trace!("got msg from {username} to dispatch. content {content}");
        let msg = ChatrMessage::ReceivedMessage { username, content };
        for (_, (_, stc)) in self.clients.iter_mut() {
            stc.send(msg.clone()).await.unwrap()
        }
    }
}
#[instrument(level = "debug", skip(new_client))]
pub async fn process_client_login(
    mut new_client: UnauthenticatedClient,
    banned_usernames: &Arc<HashSet<String>>,
) -> io::Result<ClientLoginResult> {
    let login_request = new_client.login_request().await?;
    info!(?login_request);
    let UnauthenticatedClient(socket, buf) = new_client;
    match login_request {
        ChatrMessage::LoginRequest { username } => {
            if !banned_usernames.contains(&username) {
                trace!("verif login {username}");
                Ok(ClientLoginResult::Accept(AuthenticatedClient {
                    socket,
                    buf,
                    username,
                }))
            } else {
                Ok(ClientLoginResult::Reject { socket, username })
            }
        }
        _ => todo!(),
    }
}

pub enum ClientLoginResult {
    Accept(AuthenticatedClient),
    Reject {
        socket: TcpStream,
        username: Username,
    },
}

#[derive(Debug)]
pub struct AuthenticatedClient {
    socket: TcpStream,
    buf: bytes::BytesMut,
    pub username: String,
}
impl AuthenticatedClient {
    pub async fn login_accepted(&mut self) -> io::Result<usize> {
        self.socket
            .write(
                borsh::to_vec(&ChatrMessage::LoginAccepted)
                    .unwrap()
                    .as_slice(),
            )
            .await
    }
    #[instrument(level = "debug", skip_all)]
    pub fn run(
        self,
        tx: SenderToServer,
        mut rx: ReceiverFromServer,
        cancel_token: CancellationToken,
    ) {
        let Self {
            socket,
            mut buf,
            username,
        } = self;
        let (mut socket_reader, mut socket_writer) = socket.into_split();
        let u = username.clone();
        let ct_one = cancel_token.clone();
        tokio::spawn(async move {
            tracing::debug!("spawn recv loop");
            loop {
                tokio::select! {
                    _ = ct_one.cancelled() => {
                        info!("{} cancel", u);
                        break;
                    }
                    Some(msg) = rx.recv() => {
                        trace!("{} recv from server {:?}", u, msg);
                        socket_writer
                            .write_all(&borsh::to_vec(&msg).unwrap())
                            .await
                            .unwrap();
                        socket_writer.flush().await.unwrap();
                    }
                }
            }
            info!("end writer: {}", u);
        });
        tokio::spawn(async move {
            loop {
                tracing::debug!("spawn send loop");
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("{} cancel", username);
                        break;
                    }
                bytes_read = socket_reader.read(&mut buf) => {
                        tracing::trace!("recv msg");
                        match bytes_read {
                            Ok(n) => {
                                let msg = if n!=0 {
                                    trace!("{} recv from client {:?}", username, &buf[..n]);
                                    ChatrMessage::deserialize(&mut &buf[..n]).unwrap()
                                } else {
                                    cancel_token.cancel();
                                    ChatrMessage::Disconnect
                                };
                                tracing::trace!(username, ?msg);
                                tx.send((username.clone(), msg))
                                    .await
                                    .unwrap_or_else(|x| tracing::error!(username, ?x));
                            }
                            Err(_) => todo!(),
                        }
                        }

                }
            }
        });
    }
}
#[derive(Debug)]
pub struct UnauthenticatedClient(TcpStream, bytes::BytesMut);
impl UnauthenticatedClient {
    pub fn new(stream: TcpStream) -> Self {
        Self(stream, bytes::BytesMut::zeroed(1024))
    }
    pub async fn login_request(&mut self) -> Result<ChatrMessage, std::io::Error> {
        tracing::debug!("login_request");
        self.0.read(&mut self.1).await.and_then(|n| {
            if n != 0 {
                ChatrMessage::deserialize(&mut &self.1[..n])
                    .inspect(|msg| tracing::trace!("{msg:?}"))
            } else {
                Ok(ChatrMessage::Disconnect)
            }
        })
    }
    pub async fn on_fail(mut self, reason: String) {
        let msg = ChatrMessage::LoginRejected { reason };
        self.0
            .write_all(borsh::to_vec(&msg).unwrap().as_slice())
            .await
            .unwrap();
    }
    pub async fn on_accept(self, username: String) -> AuthenticatedClient {
        let Self(socket, buf) = self;
        AuthenticatedClient {
            socket,
            buf,
            username,
        }
    }
}

pub enum LoginFlowResult {
    Accept(String),
    Reject(String),
}
