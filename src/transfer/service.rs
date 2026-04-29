use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::message::AppEvent;

use super::model::{
    OutgoingTransferRequest,
    TransferConversation,
    TransferDirection,
    TransferEvent,
    TransferRecord,
    TransferRecipient,
    TransferStatus,
};
use super::progress::ProgressThrottle;
use super::protocol::{ControlMessage, TransferOffer};
use super::receiver::{read_transfer_header, receive_transfer};
use super::sender::send_transfer;
use super::storage::{ensure_receive_root, prepare_transfer, PreparedTransfer};

static TRANSFER_COUNTER: AtomicU64 = AtomicU64::new(1);

pub const TRANSFER_CONTROL_PORT: u16 = 9001;
pub const TRANSFER_DATA_PORT: u16 = 9002;

#[derive(Clone, Debug)]
pub enum TransferCommand {
    QueueTransfer(OutgoingTransferRequest),
}

#[derive(Clone, Debug)]
struct OutgoingTransferContext {
    prepared: PreparedTransfer,
    recipient: TransferRecipient,
    conversation: TransferConversation,
}

#[derive(Clone, Debug)]
struct IncomingTransferContext {
    offer: TransferOffer,
    sender_ip: IpAddr,
    destination_root: PathBuf,
}

#[derive(Clone)]
struct TransferRuntime {
    event_tx: Sender<AppEvent>,
    outgoing_pending: Arc<Mutex<HashMap<String, OutgoingTransferContext>>>,
    incoming_pending: Arc<Mutex<HashMap<String, IncomingTransferContext>>>,
    records: Arc<Mutex<HashMap<String, TransferRecord>>>,
}

pub async fn run(mut command_rx: Receiver<TransferCommand>, event_tx: Sender<AppEvent>) {
    let runtime = TransferRuntime {
        event_tx,
        outgoing_pending: Arc::new(Mutex::new(HashMap::new())),
        incoming_pending: Arc::new(Mutex::new(HashMap::new())),
        records: Arc::new(Mutex::new(HashMap::new())),
    };

    tokio::spawn(run_control_server(runtime.clone()));
    tokio::spawn(run_data_server(runtime.clone()));

    while let Some(command) = command_rx.recv().await {
        match command {
            TransferCommand::QueueTransfer(request) => {
                let runtime = runtime.clone();
                tokio::spawn(async move {
                    handle_queue_transfer(runtime, request).await;
                });
            }
        }
    }
}

pub fn next_transfer_id() -> String {
    let now = chrono::Utc::now().timestamp_millis();
    let seq = TRANSFER_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("tr-{now}-{seq}")
}

async fn handle_queue_transfer(runtime: TransferRuntime, request: OutgoingTransferRequest) {
    let template = match prepare_transfer(&request.selection) {
        Ok(prepared) => prepared,
        Err(err) => {
            eprintln!("[transfer] Préparation échouée: {err}");
            for recipient in request.recipients {
                publish_record(
                    &runtime,
                    TransferRecord {
                        id: next_transfer_id(),
                        conversation: request.conversation.clone(),
                        peer_username: recipient.username,
                        peer_addr: recipient.addr,
                        direction: TransferDirection::Outgoing,
                        status: TransferStatus::Failed,
                        label: "Préparation du partage".to_string(),
                        total_bytes: 0,
                        transferred_bytes: 0,
                        total_files: 0,
                        transferred_files: 0,
                        current_path: None,
                        destination_root: None,
                        error: Some(err.to_string()),
                        updated_at: SystemTime::now(),
                    },
                );
            }
            return;
        }
    };

    for recipient in &request.recipients {
        let transfer_id = next_transfer_id();
        let mut prepared = template.clone();
        prepared.manifest.transfer_id = transfer_id.clone();

        let mut record = TransferRecord {
            id: transfer_id.clone(),
            conversation: request.conversation.clone(),
            peer_username: recipient.username.clone(),
            peer_addr: recipient.addr,
            direction: TransferDirection::Outgoing,
            status: TransferStatus::Preparing,
            label: prepared.manifest.label.clone(),
            total_bytes: prepared.manifest.total_bytes,
            transferred_bytes: 0,
            total_files: prepared.manifest.total_files,
            transferred_files: 0,
            current_path: None,
            destination_root: None,
            error: None,
            updated_at: SystemTime::now(),
        };
        publish_record(&runtime, record.clone());

        runtime.outgoing_pending.lock().unwrap().insert(
            transfer_id.clone(),
            OutgoingTransferContext {
                prepared: prepared.clone(),
                recipient: recipient.clone(),
                conversation: request.conversation.clone(),
            },
        );

        let message = ControlMessage::Offer(TransferOffer {
            transfer_id: transfer_id.clone(),
            from: request.from.clone(),
            conversation: request.conversation.clone(),
            manifest: prepared.manifest.clone(),
        });

        if let Err(err) = send_control_message(control_addr(recipient.addr), &message).await {
            runtime.outgoing_pending.lock().unwrap().remove(&transfer_id);
            record.status = TransferStatus::Failed;
            record.error = Some(err.to_string());
            record.updated_at = SystemTime::now();
            publish_record(&runtime, record);
            continue;
        }

        record.status = TransferStatus::WaitingForPeer;
        record.updated_at = SystemTime::now();
        publish_record(&runtime, record);
    }
}

async fn run_control_server(runtime: TransferRuntime) {
    let listener = match TcpListener::bind(("0.0.0.0", TRANSFER_CONTROL_PORT)).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("[transfer] Impossible de lancer le serveur de contrôle: {err}");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let runtime = runtime.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_control_connection(runtime, stream, peer_addr).await {
                        eprintln!("[transfer] Contrôle entrant invalide: {err}");
                    }
                });
            }
            Err(err) => eprintln!("[transfer] Accept contrôle échoué: {err}"),
        }
    }
}

async fn run_data_server(runtime: TransferRuntime) {
    let listener = match TcpListener::bind(("0.0.0.0", TRANSFER_DATA_PORT)).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("[transfer] Impossible de lancer le serveur de données: {err}");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let runtime = runtime.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_data_connection(runtime, stream, peer_addr).await {
                        eprintln!("[transfer] Réception de données échouée: {err}");
                    }
                });
            }
            Err(err) => eprintln!("[transfer] Accept données échoué: {err}"),
        }
    }
}

async fn handle_control_connection(
    runtime: TransferRuntime,
    mut stream: TcpStream,
    peer_addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    if buf.is_empty() {
        return Ok(());
    }

    let message = serde_json::from_slice::<ControlMessage>(&buf)?;
    match message {
        ControlMessage::Offer(offer) => handle_offer(runtime, offer, peer_addr).await,
        ControlMessage::Accept { transfer_id } => handle_accept(runtime, transfer_id),
        ControlMessage::Reject { transfer_id, reason } => {
            handle_remote_failure(runtime, transfer_id, reason)
        }
        ControlMessage::Completed { transfer_id } => handle_remote_completed(runtime, transfer_id),
        ControlMessage::Failed { transfer_id, message } => {
            handle_remote_failure(runtime, transfer_id, message)
        }
    }

    Ok(())
}

async fn handle_offer(
    runtime: TransferRuntime,
    offer: TransferOffer,
    peer_addr: SocketAddr,
) {
    let mut record = TransferRecord {
        id: offer.transfer_id.clone(),
        conversation: offer.conversation.clone(),
        peer_username: offer.from.clone(),
        peer_addr: data_addr(peer_addr),
        direction: TransferDirection::Incoming,
        status: TransferStatus::WaitingForPeer,
        label: offer.manifest.label.clone(),
        total_bytes: offer.manifest.total_bytes,
        transferred_bytes: 0,
        total_files: offer.manifest.total_files,
        transferred_files: 0,
        current_path: None,
        destination_root: None,
        error: None,
        updated_at: SystemTime::now(),
    };

    let destination_root = match ensure_receive_root(&offer.from, &offer.transfer_id, &offer.manifest.label) {
        Ok(path) => path,
        Err(err) => {
            record.status = TransferStatus::Failed;
            record.error = Some(err.to_string());
            publish_record(&runtime, record);
            let _ = send_control_message(
                control_addr(peer_addr),
                &ControlMessage::Reject {
                    transfer_id: offer.transfer_id,
                    reason: err.to_string(),
                },
            )
            .await;
            return;
        }
    };

    record.destination_root = Some(destination_root.clone());
    publish_record(&runtime, record);

    runtime.incoming_pending.lock().unwrap().insert(
        offer.transfer_id.clone(),
        IncomingTransferContext {
            offer: offer.clone(),
            sender_ip: peer_addr.ip(),
            destination_root,
        },
    );

    if let Err(err) = send_control_message(
        control_addr(peer_addr),
        &ControlMessage::Accept {
            transfer_id: offer.transfer_id.clone(),
        },
    )
    .await
    {
        runtime.incoming_pending.lock().unwrap().remove(&offer.transfer_id);
        let mut failed = record_from_runtime(&runtime, &offer.transfer_id).unwrap_or(TransferRecord {
            id: offer.transfer_id.clone(),
            conversation: offer.conversation,
            peer_username: offer.from,
            peer_addr: data_addr(peer_addr),
            direction: TransferDirection::Incoming,
            status: TransferStatus::Failed,
            label: offer.manifest.label,
            total_bytes: offer.manifest.total_bytes,
            transferred_bytes: 0,
            total_files: offer.manifest.total_files,
            transferred_files: 0,
            current_path: None,
            destination_root: None,
            error: None,
            updated_at: SystemTime::now(),
        });
        failed.status = TransferStatus::Failed;
        failed.error = Some(err.to_string());
        failed.updated_at = SystemTime::now();
        publish_record(&runtime, failed);
    }
}

fn handle_accept(runtime: TransferRuntime, transfer_id: String) {
    let Some(context) = runtime.outgoing_pending.lock().unwrap().remove(&transfer_id) else {
        return;
    };

    let runtime_for_task = runtime.clone();
    tokio::spawn(async move {
        let mut record = record_from_runtime(&runtime_for_task, &transfer_id).unwrap_or_else(|| {
            TransferRecord {
                id: transfer_id.clone(),
                conversation: context.conversation.clone(),
                peer_username: context.recipient.username.clone(),
                peer_addr: context.recipient.addr,
                direction: TransferDirection::Outgoing,
                status: TransferStatus::WaitingForPeer,
                label: context.prepared.manifest.label.clone(),
                total_bytes: context.prepared.manifest.total_bytes,
                transferred_bytes: 0,
                total_files: context.prepared.manifest.total_files,
                transferred_files: 0,
                current_path: None,
                destination_root: None,
                error: None,
                updated_at: SystemTime::now(),
            }
        });
        record.status = TransferStatus::Transferring;
        record.updated_at = SystemTime::now();
        publish_record(&runtime_for_task, record.clone());

        let mut throttle = ProgressThrottle::new(Duration::from_millis(150));
        let runtime_for_progress = runtime_for_task.clone();
        let mut emit_progress = |bytes: u64, files: usize, current_path: Option<String>, force: bool| {
            if throttle.should_emit(force) {
                record.transferred_bytes = bytes;
                record.transferred_files = files;
                record.current_path = current_path.clone();
                record.status = TransferStatus::Transferring;
                record.updated_at = SystemTime::now();
                publish_record(&runtime_for_progress, record.clone());
            }
        };

        match send_transfer(&context.prepared, data_addr(context.recipient.addr), &mut emit_progress).await {
            Ok(result) => {
                record.transferred_bytes = result.bytes_sent;
                record.transferred_files = result.files_sent;
                record.current_path = None;
                record.updated_at = SystemTime::now();
                publish_record(&runtime_for_task, record);
            }
            Err(err) => {
                record.status = TransferStatus::Failed;
                record.error = Some(err.to_string());
                record.updated_at = SystemTime::now();
                publish_record(&runtime_for_task, record.clone());
                let _ = send_control_message(
                    control_addr(context.recipient.addr),
                    &ControlMessage::Failed {
                        transfer_id: transfer_id.clone(),
                        message: err.to_string(),
                    },
                )
                .await;
            }
        }
    });
}

async fn handle_data_connection(
    runtime: TransferRuntime,
    mut stream: TcpStream,
    _peer_addr: SocketAddr,
) -> anyhow::Result<()> {
    let transfer_id = read_transfer_header(&mut stream).await?;
    let Some(context) = runtime.incoming_pending.lock().unwrap().remove(&transfer_id) else {
        return Ok(());
    };

    let mut record = record_from_runtime(&runtime, &transfer_id).unwrap_or(TransferRecord {
        id: transfer_id.clone(),
        conversation: context.offer.conversation.clone(),
        peer_username: context.offer.from.clone(),
        peer_addr: SocketAddr::new(context.sender_ip, TRANSFER_DATA_PORT),
        direction: TransferDirection::Incoming,
        status: TransferStatus::WaitingForPeer,
        label: context.offer.manifest.label.clone(),
        total_bytes: context.offer.manifest.total_bytes,
        transferred_bytes: 0,
        total_files: context.offer.manifest.total_files,
        transferred_files: 0,
        current_path: None,
        destination_root: Some(context.destination_root.clone()),
        error: None,
        updated_at: SystemTime::now(),
    });
    record.status = TransferStatus::Transferring;
    record.updated_at = SystemTime::now();
    publish_record(&runtime, record.clone());

    let mut throttle = ProgressThrottle::new(Duration::from_millis(150));
    let runtime_for_progress = runtime.clone();
    let mut emit_progress = |bytes: u64, files: usize, current_path: Option<String>, force: bool| {
        if throttle.should_emit(force) {
            record.transferred_bytes = bytes;
            record.transferred_files = files;
            record.current_path = current_path.clone();
            record.status = TransferStatus::Transferring;
            record.updated_at = SystemTime::now();
            publish_record(&runtime_for_progress, record.clone());
        }
    };

    match receive_transfer(
        &mut stream,
        &context.offer.manifest,
        &context.destination_root,
        &mut emit_progress,
    )
    .await
    {
        Ok(result) => {
            record.transferred_bytes = result.bytes_received;
            record.transferred_files = result.files_received;
            record.current_path = None;
            record.status = TransferStatus::Completed;
            record.updated_at = SystemTime::now();
            publish_record(&runtime, record);
            let _ = send_control_message(
                SocketAddr::new(context.sender_ip, TRANSFER_CONTROL_PORT),
                &ControlMessage::Completed { transfer_id },
            )
            .await;
        }
        Err(err) => {
            record.status = TransferStatus::Failed;
            record.error = Some(err.to_string());
            record.updated_at = SystemTime::now();
            publish_record(&runtime, record);
            let _ = send_control_message(
                SocketAddr::new(context.sender_ip, TRANSFER_CONTROL_PORT),
                &ControlMessage::Failed {
                    transfer_id,
                    message: err.to_string(),
                },
            )
            .await;
        }
    }

    Ok(())
}

fn handle_remote_completed(runtime: TransferRuntime, transfer_id: String) {
    let Some(mut record) = record_from_runtime(&runtime, &transfer_id) else {
        return;
    };

    record.status = TransferStatus::Completed;
    record.transferred_bytes = record.total_bytes;
    record.transferred_files = record.total_files;
    record.current_path = None;
    record.updated_at = SystemTime::now();
    publish_record(&runtime, record);
}

fn handle_remote_failure(runtime: TransferRuntime, transfer_id: String, reason: String) {
    runtime.outgoing_pending.lock().unwrap().remove(&transfer_id);

    let Some(mut record) = record_from_runtime(&runtime, &transfer_id) else {
        return;
    };

    record.status = TransferStatus::Failed;
    record.error = Some(reason);
    record.updated_at = SystemTime::now();
    publish_record(&runtime, record);
}

fn publish_record(runtime: &TransferRuntime, record: TransferRecord) {
    runtime.records.lock().unwrap().insert(record.id.clone(), record.clone());
    let _ = runtime
        .event_tx
        .try_send(AppEvent::Transfer(TransferEvent::Upsert(record)));
}

fn record_from_runtime(runtime: &TransferRuntime, transfer_id: &str) -> Option<TransferRecord> {
    runtime.records.lock().unwrap().get(transfer_id).cloned()
}

async fn send_control_message(target: SocketAddr, message: &ControlMessage) -> anyhow::Result<()> {
    let mut stream = TcpStream::connect(target).await?;
    let data = serde_json::to_vec(message)?;
    stream.write_all(&data).await?;
    stream.flush().await?;
    stream.shutdown().await?;
    Ok(())
}

fn control_addr(addr: SocketAddr) -> SocketAddr {
    SocketAddr::new(addr.ip(), TRANSFER_CONTROL_PORT)
}

fn data_addr(addr: SocketAddr) -> SocketAddr {
    SocketAddr::new(addr.ip(), TRANSFER_DATA_PORT)
}