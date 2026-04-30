use std::cmp::min;
use std::net::SocketAddr;

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::message::AppEvent;

use super::{
    max_header_bytes, prepare_receive_root, prepare_transfer, resolve_output_path,
    TransferDirection, TransferEntryKind, TransferManifest, TransferProgress, TransferRequest,
    TransferStatus, TRANSFER_BUFFER_SIZE, TRANSFER_PORT,
};

pub async fn run_service(event_tx: Sender<AppEvent>, mut request_rx: Receiver<TransferRequest>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", TRANSFER_PORT)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("[transfer] bind error: {}", error);
            return;
        }
    };

    loop {
        tokio::select! {
            Some(request) = request_rx.recv() => {
                let tx = event_tx.clone();
                tokio::spawn(async move {
                    if let Err(error) = send_transfer(request.clone(), tx.clone()).await {
                        let progress = TransferProgress {
                            transfer_id: format!("failed-{}-{}", request.from, chrono::Utc::now().timestamp_millis()),
                            peer: request.recipient,
                            label: "files".to_string(),
                            direction: TransferDirection::Upload,
                            status: TransferStatus::Failed,
                            bytes_done: 0,
                            total_bytes: 0,
                            current_path: None,
                            detail: error.to_string(),
                        };
                        let _ = tx.send(AppEvent::TransferUpdated(progress)).await;
                    }
                });
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        let tx = event_tx.clone();
                        tokio::spawn(async move {
                            if let Err(error) = receive_transfer(stream, tx.clone()).await {
                                eprintln!("[transfer] receive error: {}", error);
                            }
                        });
                    }
                    Err(error) => eprintln!("[transfer] accept error: {}", error),
                }
            }
        }
    }
}

async fn send_transfer(request: TransferRequest, event_tx: Sender<AppEvent>) -> Result<()> {
    let prepared = prepare_transfer(&request.from, &request.recipient, &request.paths)?;
    let transfer_addr = SocketAddr::new(request.to_addr.ip(), TRANSFER_PORT);

    emit(
        &event_tx,
        snapshot(
            &prepared.manifest,
            &request.recipient,
            TransferDirection::Upload,
            TransferStatus::Queued,
            0,
            None,
            String::new(),
        ),
    )
    .await;

    let mut stream = TcpStream::connect(transfer_addr)
        .await
        .with_context(|| format!("unable to connect to {}", transfer_addr))?;

    let header = serde_json::to_vec(&prepared.manifest)?;
    if header.len() > max_header_bytes() {
        return Err(anyhow!("transfer manifest too large"));
    }

    stream.write_u32(header.len() as u32).await?;
    stream.write_all(&header).await?;

    let mut sent_bytes = 0_u64;
    for entry in &prepared.entries {
        if entry.kind != TransferEntryKind::File {
            continue;
        }

        emit(
            &event_tx,
            snapshot(
                &prepared.manifest,
                &request.recipient,
                TransferDirection::Upload,
                TransferStatus::Running,
                sent_bytes,
                Some(entry.relative_path.clone()),
                String::new(),
            ),
        )
        .await;

        let mut file = tokio::fs::File::open(&entry.source_path)
            .await
            .with_context(|| format!("unable to open {}", entry.source_path.display()))?;
        let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];

        loop {
            let read = file.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            stream.write_all(&buffer[..read]).await?;
            sent_bytes += read as u64;
            emit(
                &event_tx,
                snapshot(
                    &prepared.manifest,
                    &request.recipient,
                    TransferDirection::Upload,
                    TransferStatus::Running,
                    sent_bytes,
                    Some(entry.relative_path.clone()),
                    String::new(),
                ),
            )
            .await;
        }
    }

    stream.flush().await?;
    stream.shutdown().await?;

    emit(
        &event_tx,
        snapshot(
            &prepared.manifest,
            &request.recipient,
            TransferDirection::Upload,
            TransferStatus::Completed,
            prepared.manifest.total_bytes,
            None,
            String::new(),
        ),
    )
    .await;

    Ok(())
}

async fn receive_transfer(mut stream: TcpStream, event_tx: Sender<AppEvent>) -> Result<()> {
    let header_len = stream.read_u32().await? as usize;
    if header_len == 0 || header_len > max_header_bytes() {
        return Err(anyhow!("invalid transfer header length {}", header_len));
    }

    let mut header = vec![0_u8; header_len];
    stream.read_exact(&mut header).await?;
    let manifest: TransferManifest = serde_json::from_slice(&header)?;
    let receive_root = prepare_receive_root(&manifest)?;
    let receive_root_label = receive_root.display().to_string();

    emit(
        &event_tx,
        snapshot(
            &manifest,
            &manifest.from,
            TransferDirection::Download,
            TransferStatus::Queued,
            0,
            None,
            receive_root_label.clone(),
        ),
    )
    .await;

    let mut received_bytes = 0_u64;
    let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];

    for entry in &manifest.entries {
        let output_path = resolve_output_path(&receive_root, &entry.relative_path)?;
        match entry.kind {
            TransferEntryKind::Directory => {
                tokio::fs::create_dir_all(&output_path).await?;
            }
            TransferEntryKind::File => {
                if let Some(parent) = output_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                emit(
                    &event_tx,
                    snapshot(
                        &manifest,
                        &manifest.from,
                        TransferDirection::Download,
                        TransferStatus::Running,
                        received_bytes,
                        Some(entry.relative_path.clone()),
                        receive_root_label.clone(),
                    ),
                )
                .await;

                let mut file = tokio::fs::File::create(&output_path)
                    .await
                    .with_context(|| format!("unable to create {}", output_path.display()))?;
                let mut remaining = entry.size_bytes;
                while remaining > 0 {
                    let chunk_len = min(remaining as usize, buffer.len());
                    let read = stream.read(&mut buffer[..chunk_len]).await?;
                    if read == 0 {
                        return Err(anyhow!(
                            "unexpected end of stream while receiving {}",
                            entry.relative_path
                        ));
                    }
                    file.write_all(&buffer[..read]).await?;
                    received_bytes += read as u64;
                    remaining = remaining.saturating_sub(read as u64);
                    emit(
                        &event_tx,
                        snapshot(
                            &manifest,
                            &manifest.from,
                            TransferDirection::Download,
                            TransferStatus::Running,
                            received_bytes,
                            Some(entry.relative_path.clone()),
                            receive_root_label.clone(),
                        ),
                    )
                    .await;
                }
            }
        }
    }

    emit(
        &event_tx,
        snapshot(
            &manifest,
            &manifest.from,
            TransferDirection::Download,
            TransferStatus::Completed,
            manifest.total_bytes,
            None,
            receive_root_label,
        ),
    )
    .await;

    Ok(())
}

async fn emit(event_tx: &Sender<AppEvent>, progress: TransferProgress) {
    let _ = event_tx.send(AppEvent::TransferUpdated(progress)).await;
}

fn snapshot(
    manifest: &TransferManifest,
    peer: &str,
    direction: TransferDirection,
    status: TransferStatus,
    bytes_done: u64,
    current_path: Option<String>,
    detail: String,
) -> TransferProgress {
    TransferProgress {
        transfer_id: manifest.transfer_id.clone(),
        peer: peer.to_string(),
        label: manifest.label.clone(),
        direction,
        status,
        bytes_done,
        total_bytes: manifest.total_bytes,
        current_path,
        detail,
    }
}