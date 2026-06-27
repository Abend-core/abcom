use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;
use tokio::time::{timeout, Duration};

use crate::config;
use crate::message::{AppEvent, NetworkPacket};

// 64 KB — bien au-delà du plus long message texte légitime (~16 000 mots)
const MAX_PACKET_SIZE: u64 = 64 * 1024;
const READ_TIMEOUT_SECS: u64 = 5;

/// Serveur TCP : écoute les connexions entrantes et dispatche les événements
pub async fn run_server(tx: Sender<AppEvent>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", config::chat_port())).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[network] Erreur de bind TCP: {}", e);
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let tx = tx.clone();
                tokio::spawn(handle_incoming(stream, tx));
            }
            Err(_) => continue,
        }
    }
}

async fn handle_incoming(stream: TcpStream, tx: Sender<AppEvent>) {
    let mut buf = Vec::new();

    let result = timeout(
        Duration::from_secs(READ_TIMEOUT_SECS),
        stream.take(MAX_PACKET_SIZE + 1).read_to_end(&mut buf),
    )
    .await;

    match result {
        Err(_) => {
            eprintln!("[network] Timeout sur la connexion entrante");
            return;
        }
        Ok(Err(e)) => {
            eprintln!("[network] Erreur de lecture: {}", e);
            return;
        }
        Ok(Ok(_)) => {}
    }

    if buf.is_empty() {
        return;
    }

    if buf.len() > MAX_PACKET_SIZE as usize {
        eprintln!(
            "[network] Paquet trop volumineux ({} bytes), ignoré",
            buf.len()
        );
        return;
    }

    match serde_json::from_slice::<NetworkPacket>(&buf) {
        Ok(NetworkPacket::Chat(msg)) => {
            let _ = tx.send(AppEvent::MessageReceived(msg)).await;
        }
        Ok(NetworkPacket::Group(event)) => {
            let _ = tx.send(AppEvent::GroupEventReceived(event)).await;
        }
        Ok(NetworkPacket::Typing(indicator)) => {
            let _ = tx.send(AppEvent::UserTyping(indicator.from)).await;
        }
        Ok(NetworkPacket::ReadReceipt(r)) => {
            let _ = tx.send(AppEvent::ReadReceiptReceived(r)).await;
        }
        Ok(NetworkPacket::Ack(ack)) => {
            let _ = tx.send(AppEvent::MessageAckReceived(ack)).await;
        }
        Ok(NetworkPacket::Avatar(announce)) => {
            let _ = tx.send(AppEvent::AvatarReceived(announce)).await;
        }
        Err(_) => eprintln!("[network] Paquet entrant non reconnu ({} bytes)", buf.len()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;

    use crate::message::{AppEvent, ChatMessage, MessageAck, NetworkPacket, ReadReceipt};

    async fn dispatch(packet: NetworkPacket) -> Option<AppEvent> {
        let (tx, mut rx) = mpsc::channel(4);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle_incoming(stream, tx).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(&serde_json::to_vec(&packet).unwrap())
            .await
            .unwrap();
        client.shutdown().await.unwrap();

        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .ok()
            .flatten()
    }

    #[tokio::test]
    async fn test_receives_chat_message() {
        let packet = NetworkPacket::Chat(ChatMessage {
            from: "alice".to_string(),
            content: "hello".to_string(),
            timestamp: "14:00".to_string(),
            timestamp_epoch: None,
            to_user: None,
            media: None,
        });
        let event = dispatch(packet).await.unwrap();
        assert!(matches!(event, AppEvent::MessageReceived(m) if m.content == "hello"));
    }

    #[tokio::test]
    async fn test_receives_read_receipt() {
        let packet = NetworkPacket::ReadReceipt(ReadReceipt {
            from: "bob".to_string(),
            to: "alice".to_string(),
            message_hash: 42,
            timestamp: "14:00".to_string(),
        });
        let event = dispatch(packet).await.unwrap();
        assert!(matches!(event, AppEvent::ReadReceiptReceived(r) if r.from == "bob"));
    }

    #[tokio::test]
    async fn test_receives_ack() {
        let packet = NetworkPacket::Ack(MessageAck {
            from: "bob".to_string(),
            to: "alice".to_string(),
            message_hash: 99,
            timestamp: "14:00".to_string(),
        });
        let event = dispatch(packet).await.unwrap();
        assert!(matches!(event, AppEvent::MessageAckReceived(a) if a.message_hash == 99));
    }

    #[tokio::test]
    async fn test_invalid_packet_dispatches_nothing() {
        let (tx, mut rx) = mpsc::channel(4);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle_incoming(stream, tx).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"NOT_VALID_JSON{{{").await.unwrap();
        client.shutdown().await.unwrap();

        let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(event.is_err() || event.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_oversized_packet_dispatches_nothing() {
        let (tx, mut rx) = mpsc::channel(4);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            super::handle_incoming(stream, tx).await;
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        let oversized = vec![b'x'; super::MAX_PACKET_SIZE as usize + 1];
        client.write_all(&oversized).await.unwrap();
        client.shutdown().await.unwrap();

        let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(event.is_err() || event.unwrap().is_none());
    }
}
