use std::io::Read;

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
            let msg_tx = msg_transmitter.clone();
            tokio::spawn(async move { process_socket(socket, msg_tx).await });
        }
    } else if args[1] == "client" {
        client().await
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
    let mut socket = TcpStream::connect("localhost:1999").await.unwrap();

    let mut rl = DefaultEditor::new().unwrap();
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
