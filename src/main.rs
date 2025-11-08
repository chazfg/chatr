use std::collections::HashMap;
use std::result::Result;
use std::sync::{Arc, Mutex};

use borsh::{BorshDeserialize, BorshSerialize};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, Sender};
use tokio::{
    io,
    net::{TcpListener, TcpStream},
    sync::mpsc::Receiver,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, trace};

#[derive(Default, Debug)]
struct Chatroom {
    clients: HashMap<Username, (CancellationToken, SenderToClient)>,
}
impl Chatroom {
    pub fn add_new_client(
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
    pub fn remove_client(&mut self, user: String) -> Option<(CancellationToken, SenderToClient)> {
        self.clients.remove(&user)
    }
}
type Username = String;
type SenderToClient = Sender<ChatrMessage>;
type SenderToServer = Sender<(Username, ChatrMessage)>;
type ReceiverFromClient = Receiver<(Username, ChatrMessage)>;
type ReceiverFromServer = Receiver<ChatrMessage>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        panic!();
    }
    info!("{}", args[1]);
    if args[1] == "server" {
        let server = TcpListener::bind("localhost:1999").await.unwrap();
        let chatroom = Arc::new(Mutex::new(Chatroom::default()));
        let (sender_to_chatroom, mut receiver_from_clients): (SenderToServer, ReceiverFromClient) =
            mpsc::channel::<(Username, ChatrMessage)>(1024);
        let chatroom_one = chatroom.clone();
        tokio::spawn(async move {
            while let Some((user, msg)) = receiver_from_clients.recv().await {
                match msg {
                    ChatrMessage::Message { content } => println!("{user}:{content}"),
                    ChatrMessage::Disconnect => {
                        let maybe_stc = chatroom_one.lock().unwrap().remove_client(user);
                        if let Some((cancel_token, stc)) = maybe_stc {
                            stc.send(ChatrMessage::Disconnect).await;
                            cancel_token.cancel();
                        }
                    }
                    _ => (),
                }
            }
        });
        loop {
            let (socket, addr) = server.accept().await.unwrap();
            tracing::debug!("new socket {}", addr);
            let mut new_client = process_client_login(UnauthenticatedClient::new(socket))
                .await
                .unwrap();
            info!("adding client: {}", new_client.username);
            new_client.login_accepted().await.unwrap();
            chatroom
                .lock()
                .unwrap()
                .add_new_client(new_client, sender_to_chatroom.clone());

            // tokio::spawn(async move { process_socket(socket, msg_tx).await });
        }
    } else if args[1] == "client" {
        client().await
    }
}
async fn process_client_login(
    mut new_client: UnauthenticatedClient,
) -> io::Result<AuthenticatedClient> {
    let login_request = new_client.login_request().await?;
    info!(?login_request);
    let UnauthenticatedClient(socket, buf) = new_client;
    match login_request {
        ChatrMessage::LoginRequest { username } => Ok(AuthenticatedClient {
            socket,
            buf,
            username,
        }),
        _ => todo!(),
    }
}

pub struct AuthenticatedClient {
    socket: TcpStream,
    buf: bytes::BytesMut,
    username: String,
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
                    }
                }
            }
            info!("end writer: {}", u);
        });
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("{} cancel", username);
                        break;
                    }
                bytes_read = socket_reader.read(&mut buf) => {
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
                // {
                //     tx.send((username.clone(), msg)).await.unwrap();
                // }
            }
        });
    }
}
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

async fn client() {
    let mut rl = DefaultEditor::new().unwrap();
    let username = match rl.readline("username?") {
        Ok(read_name) => read_name,
        Err(e) => panic!("{e}"),
    };
    let socket = TcpStream::connect("localhost:1999").await.unwrap();
    let (mut read_socket, mut write_socket) = socket.into_split();

    let mut read_buf = bytes::BytesMut::zeroed(1024);
    let m = borsh::to_vec(&ChatrMessage::LoginRequest { username }).unwrap();
    write_socket.write_all(m.as_slice()).await.unwrap();
    tokio::spawn(async move {
        loop {
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    rl.add_history_entry(line.as_str()).unwrap();
                    write_socket
                        .write_all(
                            &borsh::to_vec(&ChatrMessage::Message { content: line }).unwrap(),
                        )
                        .await
                        .unwrap();
                }
                Err(ReadlineError::Interrupted) => {
                    tracing::error!("CTRL-C");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    tracing::error!("CTRL-D");
                    break;
                }
                Err(err) => {
                    tracing::error!("Error: {:?}", err);
                    break;
                }
            }
        }
    });
    loop {
        if let Ok(msg) = read_socket.read(&mut read_buf).await.and_then(|n| {
            if n != 0 {
                ChatrMessage::deserialize(&mut &read_buf[..n])
            } else {
                Ok(ChatrMessage::Disconnect)
            }
        }) {
            println!("{msg:?}");
            if matches!(msg, ChatrMessage::Disconnect) {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub enum ChatrMessage {
    LoginRequest { username: String },
    LoginAccepted,
    LoginRejected { reason: String },
    Message { content: String },
    Disconnect,
}
