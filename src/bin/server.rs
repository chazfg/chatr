use std::sync::Arc;

use chatr::{
    ChatrMessage, ReceiverFromClient, SenderToServer, Username,
    chatroom::{AdminMsg, Chatroom, UnauthenticatedClient, process_client_login},
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, mpsc},
};
use tokio_util::sync::CancellationToken;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let server = TcpListener::bind("localhost:1999").await.unwrap();
    let chatroom = Chatroom::default();
    let (sender_to_chatroom, mut receiver_from_clients): (SenderToServer, ReceiverFromClient) =
        mpsc::channel::<(Username, ChatrMessage)>(1024);
    let (admin_send, admin_recv) = mpsc::channel::<AdminMsg>(1024);
    chatroom.run(admin_recv);
    let admin_send_one = admin_send.clone();
    tokio::spawn(async move {
        while let Some((user, msg)) = receiver_from_clients.recv().await {
            tracing::trace!("recv from {user} msg {msg:?}");
            match msg {
                ChatrMessage::SentMessage { content } => {
                    admin_send_one
                        .send(AdminMsg::DispatchMsg(user, content))
                        .await
                        .unwrap()
                    // chatroom_one.lock().await.dispatch_msg(user, content).await;
                }
                ChatrMessage::Disconnect => {
                    admin_send_one
                        .send(AdminMsg::RemoveClient(user))
                        .await
                        .unwrap();
                    // let maybe_stc = chatroom_one.lock().await.remove_client(user).await;
                    // if let Some((cancel_token, stc)) = maybe_stc {
                    //     stc.send(ChatrMessage::Disconnect).await.unwrap();
                    //     cancel_token.cancel();
                    // }
                }
                _ => (),
            }
        }
    });
    tokio::spawn(async move {
        loop {
            let (socket, addr) = server.accept().await.unwrap();
            // let chat_link = chatroom.clone();
            let send_link = sender_to_chatroom.clone();
            let admin_send = admin_send.clone();
            // tokio::spawn(async move {
            tracing::debug!("new socket {}", addr);
            let maybe_new_client = process_client_login(UnauthenticatedClient::new(socket)).await;
            let mut new_client = match maybe_new_client {
                Ok(yes) => {
                    tracing::info!("adding client: {}", yes.username);
                    yes
                }
                Err(e) => {
                    tracing::error!("fucked up {e}");
                    return;
                }
            };
            let user = new_client.username.clone();
            new_client.login_accepted().await.unwrap();
            let (client_send, client_recv) = mpsc::channel(1024);
            tracing::debug!("run {user}");
            new_client.run(send_link, client_recv, CancellationToken::new());
            admin_send
                .send(AdminMsg::AddClient(user, client_send))
                .await
                .unwrap();
            // });

            // tokio::spawn(async move { process_socket(socket, msg_tx).await });
        }
    });
    tokio::signal::ctrl_c().await;
}
