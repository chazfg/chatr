use std::io::Read;
use std::result::Result;

use borsh::{BorshDeserialize, BorshSerialize};
use bytes::BufMut;
use rustyline::DefaultEditor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, Sender};
use tokio::{
    io,
    net::{TcpListener, TcpStream},
    sync::mpsc::Receiver,
};
#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        panic!();
    }
    if args[1] == "server" {
        let server = TcpListener::bind("localhost:1999").await.unwrap();
        let (msg_transmitter, mut msg_receiver) = mpsc::channel::<bytes::Bytes>(1024);
        tokio::spawn(async move {
            while let Some(m) = msg_receiver.recv().await {
                println!("{m:?}");
            }
        });
        loop {
            let (socket, _) = server.accept().await.unwrap();
            let new_client = UnauthenticatedClient::new(socket);

            let msg_tx = msg_transmitter.clone();
            // tokio::spawn(async move { process_socket(socket, msg_tx).await });
        }
    } else if args[1] == "client" {
        client().await
    }
}
pub struct UnauthenticatedClient(TcpStream, bytes::BytesMut);
pub struct AuthenticatedClient {
    socket: TcpStream,
    buf: bytes::BytesMut,
    username: String,
}
impl UnauthenticatedClient {
    pub fn new(stream: TcpStream) -> Self {
        Self(stream, bytes::BytesMut::zeroed(1024))
    }
    pub async fn login_request(&mut self) -> Result<ChatrMessage, std::io::Error> {
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
            .write(borsh::to_vec(&msg).unwrap().as_slice())
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

async fn process_client(
    mut new_client: UnauthenticatedClient,
    chatroom_tx: Sender<bytes::Bytes>,
) -> Option<AuthenticatedClient> {
    let received_message = new_client.login_request().await;
    let result = match received_message {
        Ok(chatr_msg) => match chatr_msg {
            ChatrMessage::LoginRequest { username } => LoginFlowResult::Accept(username),
            _ => LoginFlowResult::Reject("Wrong message".to_string()),
        },
        Err(e) => LoginFlowResult::Reject("Some error".to_string()),
    };
    match result {
        LoginFlowResult::Accept(s) => Some(new_client.on_accept(s).await),
        LoginFlowResult::Reject(s) => {
            new_client.on_fail(s).await;
            None
        }
    }
}
async fn process_socket(mut socket: TcpStream, msg_tx: Sender<bytes::Bytes>) {
    let mut read_buffer = bytes::BytesMut::zeroed(1024);
    loop {
        match socket.read(&mut read_buffer).await {
            Ok(0) => todo!("disconnect"),
            Ok(n) => {
                msg_tx
                    .send(read_buffer.iter().take(n).copied().collect())
                    .await
                    .unwrap();
            }
            Err(_) => todo!(),
        };
    }
}

fn post_to_stdout(mut reader: Receiver<bytes::Bytes>) {
    let mut stdout = io::stdout();
    tokio::spawn(async move {
        while let Some(r) = reader.recv().await {
            match stdout.write(&r).await {
                Ok(_) => (),
                Err(_) => (),
            }
        }
    });
}
async fn client() {
    let mut rl = DefaultEditor::new().unwrap();
    let username = match rl.readline("username?") {
        Ok(read_name) => read_name,
        Err(e) => panic!("{e}"),
    };
    let mut socket = TcpStream::connect("localhost:1999").await.unwrap();
    let mut write_buf = bytes::BytesMut::zeroed(1024);
    let m = borsh::to_vec(&ChatrMessage::LoginRequest { username }).unwrap();
    socket.write(m.as_slice()).await;
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                socket.write(line.as_bytes()).await.unwrap();
            }
            Err(_) => todo!(),
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
