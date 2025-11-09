use std::{collections::HashSet, path::Path, sync::Arc};

use chatr::{
    ChatrMessage, ReceiverFromClient, SenderToServer, Username,
    chatroom::{
        AdminMsg, Chatroom, ClientLoginResult, UnauthenticatedClient, process_client_login,
    },
};
use clap::Parser;
use tokio::{io::AsyncWriteExt, net::TcpListener, sync::mpsc};
use tokio_util::sync::CancellationToken;

#[derive(clap::Parser, Debug, Clone)]
struct ServerArgs {
    host: String,
    banned_usernames: Option<String>,
}

/// Server binary
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    // Get clargs
    let ServerArgs {
        banned_usernames,
        host,
    } = ServerArgs::parse();
    let banned_usernames: Arc<HashSet<String>> = Arc::new(match banned_usernames {
        Some(string) => {
            let as_path = Path::new(&string);
            if as_path.exists() {
                let file_str = std::fs::read_to_string(as_path).unwrap();
                HashSet::from_iter(file_str.split(",").map(|s| s.trim().to_string()))
            } else {
                HashSet::from_iter(string.split(",").map(|s| s.trim().to_string()))
            }
        }
        None => HashSet::default(),
    });

    // Bind to host, create chatroom
    let server = TcpListener::bind(host).await.unwrap();
    let chatroom = Chatroom::new();
    // Channels for comms
    let (sender_to_chatroom, mut receiver_from_clients): (SenderToServer, ReceiverFromClient) =
        mpsc::channel::<(Username, ChatrMessage)>(1024);
    let (admin_send, admin_recv) = mpsc::channel::<AdminMsg>(1024);
    // Run chatroom
    chatroom.run(admin_recv);
    let admin_send_one = admin_send.clone();
    // Fan in listener for all clients/users
    tokio::spawn(async move {
        while let Some((user, msg)) = receiver_from_clients.recv().await {
            tracing::trace!("recv from {user} msg {msg:?}");
            match msg {
                ChatrMessage::SentMessage { content } => admin_send_one
                    .send(AdminMsg::DispatchMsg(user, content))
                    .await
                    .unwrap(),
                ChatrMessage::Disconnect => {
                    admin_send_one
                        .send(AdminMsg::RemoveClient(user))
                        .await
                        .unwrap();
                }
                _ => (),
            }
        }
    });
    // Socket listener accepting new connections
    tokio::spawn(async move {
        loop {
            let (socket, addr) = server.accept().await.unwrap();
            let send_link = sender_to_chatroom.clone();
            let admin_send = admin_send.clone();
            tracing::debug!("new socket {}", addr);
            let maybe_new_client =
                process_client_login(UnauthenticatedClient::new(socket), &banned_usernames).await;
            let mut new_client = match maybe_new_client {
                Ok(result) => match result {
                    ClientLoginResult::Accept(authenticated_client) => {
                        tracing::info!("adding client: {}", authenticated_client.username);
                        authenticated_client
                    }
                    ClientLoginResult::Reject {
                        mut socket,
                        username,
                    } => {
                        socket
                            .write_all(
                                &borsh::to_vec(&ChatrMessage::LoginRejected {
                                    reason: format!("{username} is not allowed"),
                                })
                                .unwrap(),
                            )
                            .await
                            .unwrap();
                        return;
                    }
                },
                Err(e) => {
                    tracing::error!("fucked up {e}");
                    return;
                }
            };
            let user = new_client.username.clone();
            new_client.login_accepted().await.unwrap();
            let (client_send, client_recv) = mpsc::channel(1024);
            tracing::debug!("run {user}");
            // Client spawned when verified
            new_client.run(send_link, client_recv, CancellationToken::new());
            admin_send
                .send(AdminMsg::AddClient(user, client_send))
                .await
                .unwrap();
        }
    });
    tokio::signal::ctrl_c().await.unwrap();
}
