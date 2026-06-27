use std::cmp::min;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

use crate::config;
use crate::message::AppEvent;

use super::{
    max_header_bytes, prepare_transfer, resolve_output_path, TransferDecision, TransferDirection,
    TransferEntryKind, TransferManifest, TransferOffer, TransferProgress, TransferRequest,
    TransferStatus, TRANSFER_BUFFER_SIZE,
};

/// Délai max d'attente d'une décision accepter/refuser (de part et d'autre).
const DECISION_TIMEOUT: Duration = Duration::from_secs(120);

pub async fn run_service(
    event_tx: Sender<AppEvent>,
    offer_tx: Sender<TransferOffer>,
    mut request_rx: Receiver<TransferRequest>,
) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", config::transfer_port())).await {
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
                        let offers = offer_tx.clone();
                        tokio::spawn(async move {
                            if let Err(error) = receive_transfer(stream, tx.clone(), offers).await {
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
    // Le port de transfert du pair = son port de chat + 1 (cf. config::transfer_port).
    let transfer_addr = SocketAddr::new(request.to_addr.ip(), request.to_addr.port() + 1);

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
    stream.flush().await?;

    // Attendre la décision du destinataire : 1 = accepté, 0 = refusé.
    let accepted = matches!(
        tokio::time::timeout(DECISION_TIMEOUT, stream.read_u8()).await,
        Ok(Ok(1))
    );
    if !accepted {
        emit(
            &event_tx,
            snapshot(
                &prepared.manifest,
                &request.recipient,
                TransferDirection::Upload,
                TransferStatus::Rejected,
                0,
                None,
                String::new(),
            ),
        )
        .await;
        return Ok(());
    }

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

async fn receive_transfer(
    mut stream: TcpStream,
    event_tx: Sender<AppEvent>,
    offer_tx: Sender<TransferOffer>,
) -> Result<()> {
    let header_len = stream.read_u32().await? as usize;
    if header_len == 0 || header_len > max_header_bytes() {
        return Err(anyhow!("invalid transfer header length {}", header_len));
    }

    let mut header = vec![0_u8; header_len];
    stream.read_exact(&mut header).await?;
    let manifest: TransferManifest = serde_json::from_slice(&header)?;

    // Proposer le transfert à l'utilisateur et attendre sa décision avant
    // d'écrire le moindre octet sur le disque.
    let (decision_tx, decision_rx) = oneshot::channel::<TransferDecision>();
    let offer = TransferOffer {
        transfer_id: manifest.transfer_id.clone(),
        from: manifest.from.clone(),
        label: manifest.label.clone(),
        total_bytes: manifest.total_bytes,
        item_count: manifest.item_count,
        decision_tx,
    };
    if offer_tx.send(offer).await.is_err() {
        // Pas d'UI pour décider → refus par sécurité.
        let _ = stream.write_u8(0).await;
        return Ok(());
    }

    let decision = match tokio::time::timeout(DECISION_TIMEOUT, decision_rx).await {
        Ok(Ok(decision)) => decision,
        _ => TransferDecision {
            accept: false,
            dest_dir: None,
        },
    };

    // Informer l'émetteur (1 = accepté, 0 = refusé).
    stream.write_u8(if decision.accept { 1 } else { 0 }).await?;
    stream.flush().await?;

    if !decision.accept {
        emit(
            &event_tx,
            snapshot(
                &manifest,
                &manifest.from,
                TransferDirection::Download,
                TransferStatus::Rejected,
                0,
                None,
                String::new(),
            ),
        )
        .await;
        return Ok(());
    }

    let receive_root = decision
        .dest_dir
        .ok_or_else(|| anyhow!("aucun dossier de destination sélectionné"))?;
    tokio::fs::create_dir_all(&receive_root)
        .await
        .with_context(|| format!("unable to create {}", receive_root.display()))?;
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;

    use crate::message::AppEvent;
    use crate::transfer::{
        TransferDecision, TransferDirection, TransferEntry, TransferEntryKind, TransferManifest,
        TransferOffer, TransferRequest, TransferStatus,
    };

    fn manifest(from: &str, to: &str, filename: &str, size: u64) -> TransferManifest {
        TransferManifest {
            transfer_id: "test-42".to_string(),
            from: from.to_string(),
            to: to.to_string(),
            label: filename.to_string(),
            item_count: 1,
            total_bytes: size,
            entries: vec![TransferEntry {
                relative_path: filename.to_string(),
                kind: TransferEntryKind::File,
                size_bytes: size,
            }],
        }
    }

    #[test]
    fn test_snapshot_fields() {
        let m = manifest("alice", "bob", "doc.txt", 100);
        let p = super::snapshot(
            &m,
            "bob",
            TransferDirection::Upload,
            TransferStatus::Running,
            42,
            Some("doc.txt".to_string()),
            String::new(),
        );
        assert_eq!(p.transfer_id, "test-42");
        assert_eq!(p.peer, "bob");
        assert_eq!(p.total_bytes, 100);
        assert_eq!(p.bytes_done, 42);
        assert_eq!(p.direction, TransferDirection::Upload);
        assert_eq!(p.status, TransferStatus::Running);
        assert_eq!(p.current_path, Some("doc.txt".to_string()));
    }

    #[tokio::test]
    async fn test_receive_rejects_zero_header_length() {
        let (event_tx, _) = mpsc::channel(4);
        let (offer_tx, _) = mpsc::channel(4);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::receive_transfer(stream, event_tx, offer_tx).await
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_u32(0).await.unwrap();
        drop(client);

        let result = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(result.is_err(), "header_len=0 doit retourner une erreur");
    }

    #[tokio::test]
    async fn test_receive_refuses_when_no_ui() {
        // offer_tx sans receiver → l'UI est absente → refus automatique (envoi 0)
        let (event_tx, _) = mpsc::channel(4);
        let (offer_tx, offer_rx) = mpsc::channel(1);
        drop(offer_rx);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let m = manifest("alice", "bob", "f.txt", 5);
        let header = serde_json::to_vec(&m).unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = super::receive_transfer(stream, event_tx, offer_tx).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_u32(header.len() as u32).await.unwrap();
        client.write_all(&header).await.unwrap();

        let response = tokio::time::timeout(Duration::from_secs(2), client.read_u8())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response, 0, "doit envoyer 0 quand l'UI est absente");
    }

    #[tokio::test]
    async fn test_receive_user_rejects() {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let (offer_tx, mut offer_rx) = mpsc::channel(1);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let m = manifest("alice", "bob", "f.txt", 5);
        let header = serde_json::to_vec(&m).unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = super::receive_transfer(stream, event_tx, offer_tx).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_u32(header.len() as u32).await.unwrap();
        client.write_all(&header).await.unwrap();

        let offer: TransferOffer = tokio::time::timeout(Duration::from_secs(2), offer_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(offer.from, "alice");
        offer
            .decision_tx
            .send(TransferDecision {
                accept: false,
                dest_dir: None,
            })
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(2), client.read_u8())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response, 0, "doit envoyer 0 quand l'utilisateur refuse");

        let event = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();
        match event {
            AppEvent::TransferUpdated(p) => {
                assert_eq!(p.status, TransferStatus::Rejected)
            }
            _ => panic!("Attendu TransferUpdated(Rejected)"),
        }
    }

    #[tokio::test]
    async fn test_full_transfer_round_trip() {
        let pid = std::process::id();
        let src_dir = std::env::temp_dir().join(format!("abcom-send-{}", pid));
        let dest_dir = std::env::temp_dir().join(format!("abcom-recv-{}", pid));
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("hello.txt"), b"hello").unwrap();

        let (send_event_tx, mut send_event_rx) = mpsc::channel(16);
        let (recv_event_tx, _) = mpsc::channel(16);
        let (offer_tx, mut offer_rx) = mpsc::channel(1);

        // Le listener reçoit le transfert
        let recv_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let recv_port = recv_listener.local_addr().unwrap().port();

        let recv_task = tokio::spawn(async move {
            let (stream, _) = recv_listener.accept().await.unwrap();
            super::receive_transfer(stream, recv_event_tx, offer_tx).await
        });

        // Auto-accepter l'offre
        let dest_dir_clone = dest_dir.clone();
        tokio::spawn(async move {
            if let Some(offer) = offer_rx.recv().await {
                let _ = offer.decision_tx.send(TransferDecision {
                    accept: true,
                    dest_dir: Some(dest_dir_clone),
                });
            }
        });

        // send_transfer fait to_addr.port() + 1 pour joindre le service de transfert.
        // Le listener est sur recv_port → on passe recv_port - 1 comme port de chat.
        let to_addr: std::net::SocketAddr =
            format!("127.0.0.1:{}", recv_port - 1).parse().unwrap();
        let request = TransferRequest {
            from: "alice".to_string(),
            recipient: "bob".to_string(),
            to_addr,
            paths: vec![src_dir.join("hello.txt")],
        };

        let result = super::send_transfer(request, send_event_tx).await;
        assert!(result.is_ok(), "send_transfer a échoué : {:?}", result.err());

        // Attendre que receive_transfer ait fini d'écrire sur disque
        tokio::time::timeout(Duration::from_secs(5), recv_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        // Vérifier l'événement Completed
        let mut completed = false;
        loop {
            match tokio::time::timeout(Duration::from_millis(200), send_event_rx.recv()).await {
                Ok(Some(AppEvent::TransferUpdated(p))) if p.status == TransferStatus::Completed => {
                    completed = true;
                    break;
                }
                Ok(Some(_)) => continue,
                _ => break,
            }
        }
        assert!(completed, "Aucun événement Completed reçu côté envoi");

        // Vérifier le fichier reçu sur disque
        let received = dest_dir.join("hello.txt");
        assert!(received.exists(), "Fichier non créé à destination");
        assert_eq!(std::fs::read(&received).unwrap(), b"hello");

        let _ = std::fs::remove_dir_all(&src_dir);
        let _ = std::fs::remove_dir_all(&dest_dir);
    }
}
