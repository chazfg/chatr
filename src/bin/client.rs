use std::sync::Arc;

use bytes::BytesMut;
use chatr::{ChatrMessage, ClientConnection};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let mut stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    let ct = CancellationToken::new();
    let mut line = String::new();
    // stdout.write(b">> ").await.unwrap();
    // let mut rl = DefaultEditor::new().unwrap();
    stdout.write_all(b"username?").await.unwrap();
    stdout.flush().await;
    let username = match reader.read_line(&mut line).await {
        Ok(0) => {
            eprintln!("can't do no username");
            return;
        }
        Ok(n) => {
            let username = std::mem::take(&mut line);
            if username.trim().is_empty() {
                eprintln!("can't do no username");
                return;
            }
            username.trim().to_string()
        }
        Err(_) => todo!(),
    };

    let mut client_conn = ClientConnection::new("localhost:1999").await.unwrap();
    client_conn.login(username).await.unwrap();
    let (s1, r1) = mpsc::channel(1024);
    let (s2, r2) = mpsc::channel(1024);

    spawn_rest(stdout, reader, line, s1, r2, ct.clone());
    client_conn.run(s2, r1, ct.clone());

    tokio::signal::ctrl_c().await;
    ct.cancel();
}

fn spawn_rest(
    mut stdout: tokio::io::Stdout,
    mut stdin: tokio::io::BufReader<tokio::io::Stdin>,
    mut read_buf: String,
    s1: Sender<ChatrMessage>,
    mut r2: Receiver<ChatrMessage>,
    ct: CancellationToken,
) {
    let arc_stdout = Arc::new(Mutex::new(stdout));
    let astd = arc_stdout.clone();
    tokio::spawn(async move {
        loop {
            {
                let mut lockout = astd.lock().await;
                lockout.write_all(b">> ").await.unwrap();
                lockout.flush().await;
            }
            tokio::select! {
                _= ct.cancelled() => {break}
            res = stdin.read_line(&mut read_buf) => match res {
                Ok(0) => (),
                Ok(_) => {

                    let content = std::mem::take(&mut read_buf);

                        if !content.trim().is_empty() {
                    s1.send(ChatrMessage::SentMessage { content: content.trim().to_string() })
                        .await
                        .unwrap();

                        }
                }
                Err(_) => todo!(),
            }
            }
        }
    });
    tokio::spawn(async move {
        while let Some(msg) = r2.recv().await {
            tracing::trace!("got msg from server {msg:?}");
            match msg {
                ChatrMessage::ReceivedMessage { username, content } => {
                    let mut lockout = arc_stdout.lock().await;
                    lockout
                        .write_all(format!("{username}:{content}\n").as_bytes())
                        .await
                        .unwrap();
                    lockout.flush().await;
                }
                ChatrMessage::Disconnect => break,
                _ => panic!(),
            }
        }
    });
}
