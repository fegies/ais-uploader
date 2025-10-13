mod ais_reformatter;

use std::{
    convert::Infallible, error::Error, net::SocketAddr, pin::pin, sync::Arc, time::Duration,
};

use ais_reformatter::process_complete_chunk;
use clap::Parser;
use reqwest::{Body, Client, Method, Request, RequestBuilder, Url};
use tokio::{
    io::AsyncReadExt,
    net::TcpStream,
    sync::{
        broadcast,
        mpsc::{self, Sender},
    },
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// This program listens on a tcp/udp port and forwards received AIS data to the configured
/// endpoint.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[arg(short = 'e', long, env = "UPLOAD_ENDPOINT")]
    upload_endpoint: Url,

    #[arg(short, long, env = "AUTH_TOKEN")]
    auth_token: String,

    #[clap(flatten)]
    ports: ListenPorts,

    /// write all messages to be forwarded to standard out in addition to forwarding
    #[arg(short = 'l', long)]
    write_to_stdout: bool,

    /// prefix received lines with the current unix timestamp
    #[arg(short = 'p', long)]
    prefix_current_time: bool,
}

#[derive(Parser, Debug)]
#[group(required = true, multiple = true)]
struct ListenPorts {
    /// listen on the specified udp port for ais messages.
    /// Expect a single AIS message per packet
    #[arg(short = 'u', long)]
    udp_listener: Option<SocketAddr>,

    /// listen on the specified TCP port for ais messages.
    #[arg(short = 't', long)]
    tcp_listener: Option<SocketAddr>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    debug!("read args: {args:?}");
    let shutdown_token = register_ctrl_c_listener();

    let (msg_tx, msg_rx) = tokio::sync::mpsc::channel(100);

    if let Some(addr) = args.ports.udp_listener {
        let socket = tokio::net::UdpSocket::bind(addr).await?;
        info!("listening on UDP addr {addr}");
        let msg_tx = msg_tx.clone();
        let udp_handle = tokio::task::spawn(async move {
            let res = run_udp_listener(socket, msg_tx, args.prefix_current_time).await;
            info!("Udp listener exited with result: {res:?}");
        });

        let udp_abort_handle = udp_handle.abort_handle();
        let shutdown_token = shutdown_token.clone();
        tokio::task::spawn(async move {
            shutdown_token.cancelled().await;
            udp_abort_handle.abort();
        });
    }

    if let Some(addr) = args.ports.tcp_listener {
        let socket = tokio::net::TcpListener::bind(addr).await?;
        info!("listening on TCP addr {addr}");

        let shutdown_token = shutdown_token.clone();
        let msg_tx = msg_tx.clone();
        tokio::task::spawn(async move {
            let res = run_tcp_listener(socket, msg_tx, shutdown_token).await;

            info!("tcp listener exited with result {res:?}");
        });
    }

    drop(msg_tx);

    let upload_handle = tokio::task::spawn(run_upload(
        shutdown_token,
        msg_rx,
        args.upload_endpoint,
        args.auth_token.into(),
    ));

    upload_handle.await.unwrap().expect("upload failed");

    Ok(())
}

async fn run_udp_listener(
    socket: tokio::net::UdpSocket,
    msg_tx: Sender<Vec<u8>>,
    add_time_prefix: bool,
) -> Result<(), std::io::Error> {
    let mut buf = [0u8; 4096];
    loop {
        let num_bytes = socket.recv(&mut buf).await?;
        for line in process_complete_chunk(&buf[..num_bytes], add_time_prefix) {
            msg_tx
                .send(line)
                .await
                .expect("channel closed unexpectedly");
        }
    }
}

async fn run_tcp_listener(
    socket: tokio::net::TcpListener,
    msg_tx: Sender<Vec<u8>>,
    shutdown_token: CancellationToken,
) -> Result<(), Box<dyn Error>> {
    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                break;
            },
            accept_res = socket.accept() => {
                let (conn, peer_addr) = accept_res?;
                info!("accepted TCP connection for peer {peer_addr}");
                let shutdown_token = shutdown_token.clone();
                let msg_tx = msg_tx.clone();
                tokio::task::spawn(async move {
                    let res = shutdown_token.run_until_cancelled(process_tcp_stream(conn, msg_tx)).await;
                });
            }
        }
    }

    Ok(())
}

async fn process_tcp_stream(
    mut conn: TcpStream,
    msg_tx: Sender<Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; 4096];
    loop {
        conn.read(&mut buf).await?;
    }
}

async fn run_upload(
    shutdown_token: CancellationToken,
    mut message_rx: mpsc::Receiver<Vec<u8>>,
    url: Url,
    auth_token: Arc<str>,
) -> Result<(), Box<dyn std::error::Error + Send>> {
    let mut recovered_msg = None;

    loop {
        let (upload_tx, upload_rx) = tokio::sync::mpsc::channel(1);
        let mut upload_task = pin!(run_single_request(
            upload_rx,
            url.clone(),
            auth_token.clone()
        ));

        let mut recycle_timeout = pin!(tokio::time::sleep(Duration::from_secs(60 * 5)));

        if let Some(msg) = recovered_msg.take() {
            upload_tx
                .send(msg)
                .await
                .expect("new channel should have space for at least 1 msg");
        }

        loop {
            tokio::select! {
                msg_result = message_rx.recv() => {
                    if let Some(msg) = msg_result {
                        if let Err(er) = upload_tx.send(Ok(msg)).await {
                            recovered_msg = Some(er.0);
                            break;
                        }
                    }
                    else {
                        info!("upload stream source shut down, stopping upload");
                        drop(upload_tx);
                        return upload_task.await;
                    }
                },
                _ = &mut recycle_timeout => {
                    info!("recycling connection");
                    drop(upload_tx);
                    upload_task.await?;
                    break;
                }
                upload_result = &mut upload_task => {
                    if let Err(e) = upload_result {
                        error!("error uploading: {e}");
                        info!("waiting 15 secs before retrying");
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(15)) => {},
                            _ = shutdown_token.cancelled() => {
                                return Ok(());
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
}

async fn run_single_request(
    message_rx: mpsc::Receiver<Result<Vec<u8>, Infallible>>,
    url: Url,
    auth_token: Arc<str>,
) -> Result<(), Box<dyn std::error::Error + Send + 'static>> {
    let client = Client::new();
    let req = RequestBuilder::from_parts(Client::new(), Request::new(Method::POST, url))
        .bearer_auth(auth_token)
        .body(Body::wrap_stream(ReceiverStream::new(message_rx)))
        .build()
        .expect("Failed to build request");

    info!("launching upload connection");
    client
        .execute(req)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| {
            let v: Box<dyn std::error::Error + Send> = Box::new(e);
            v
        })?;

    Ok(())
}

fn register_ctrl_c_listener() -> CancellationToken {
    let shutdown_token = CancellationToken::new();
    let cloned_token = shutdown_token.clone();
    _ = tokio::task::spawn(async move {
        info!("Set up ctrl_c handler");
        tokio::signal::ctrl_c()
            .await
            .expect("could not set up exit handler");
        info!("Shutdown requested");
        cloned_token.cancel();
    });

    shutdown_token
}
