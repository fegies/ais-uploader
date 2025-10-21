use std::{convert::Infallible, io::Write, pin::pin, sync::Arc, time::Duration};

use reqwest::{Body, Client, Method, Request, RequestBuilder, Url};
use tokio::{sync::mpsc, time::Instant};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub async fn run_upload(
    shutdown_token: CancellationToken,
    mut message_rx: mpsc::Receiver<Vec<u8>>,
    url: Url,
    auth_token: Arc<str>,
    write_to_stdout: bool,
) -> Result<(), Box<dyn std::error::Error + Send>> {
    let mut recovered_msg = None;
    let client = Client::new();

    loop {
        let (upload_tx, upload_rx) = tokio::sync::mpsc::channel(1);
        let mut upload_task = pin!(run_single_request(
            client.clone(),
            upload_rx,
            url.clone(),
            auth_token.clone()
        ));
        let upload_start_instant = Instant::now();

        let mut recycle_timeout = pin!(tokio::time::sleep(Duration::from_secs(55)));

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
                        if write_to_stdout {
                            _ = std::io::stdout().write_all(&msg);
                        }

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
                        let wait_deadline = upload_start_instant + Duration::from_secs(15);
                        if wait_deadline > Instant::now() {
                            info!("waiting until {wait_deadline:?} before retrying");
                            tokio::select! {
                                _ = tokio::time::sleep_until(wait_deadline) => {},
                                _ = shutdown_token.cancelled() => {
                                    return Ok(());
                                }
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
    client: Client,
    message_rx: mpsc::Receiver<Result<Vec<u8>, Infallible>>,
    url: Url,
    auth_token: Arc<str>,
) -> Result<(), Box<dyn std::error::Error + Send + 'static>> {
    let (client, req_r) = RequestBuilder::from_parts(client, Request::new(Method::POST, url))
        .bearer_auth(auth_token)
        .body(Body::wrap_stream(ReceiverStream::new(message_rx)))
        .build_split();
    let req = req_r.expect("Failed to build request");

    info!("launching upload connection");

    let res = tokio::time::timeout(Duration::from_secs(90), client.execute(req))
        .await
        .box_err()
        .inspect_err(|_| warn!("Request deadline expired!"))?
        .and_then(|r| r.error_for_status())
        .box_err()?;

    info!("upload finished cleanly with status {res:?}");

    Ok(())
}

trait BoxErr<T, R> {
    fn box_err(self: Self) -> Result<T, Box<dyn std::error::Error + Send + 'static>>;
}

impl<T, R> BoxErr<T, R> for Result<T, R>
where
    R: std::error::Error + Send + 'static,
{
    fn box_err(self: Self) -> Result<T, Box<dyn std::error::Error + Send + 'static>> {
        self.map_err(|e| {
            let v: Box<dyn std::error::Error + Send + 'static> = Box::new(e);
            v
        })
    }
}
