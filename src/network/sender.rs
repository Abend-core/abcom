use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;

use crate::message::{
    AvatarRequest, MessageAckRequest, NetworkPacket, ReadReceiptRequest, SendGroupRequest,
    SendRequest, TypingRequest,
};

pub(crate) async fn send_packet(addr: std::net::SocketAddr, packet: NetworkPacket) {
    match TcpStream::connect(addr).await {
        Ok(mut stream) => {
            if let Ok(data) = serde_json::to_vec(&packet) {
                let _ = stream.write_all(&data).await;
                let _ = stream.flush().await;
                let _ = stream.shutdown().await;
            }
        }
        Err(e) => eprintln!("[network] Connexion échouée vers {}: {}", addr, e),
    }
}

/// Expéditeur TCP pour les messages de chat
pub async fn run_sender(mut rx: Receiver<SendRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Chat(req.message);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les événements de groupe
pub async fn run_sender_group(mut rx: Receiver<SendGroupRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Group(req.event);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les indicateurs de frappe (fire-and-forget)
pub async fn run_sender_typing(mut rx: Receiver<TypingRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Typing(req.indicator);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les accusés de lecture
pub async fn run_sender_read_receipts(mut rx: Receiver<ReadReceiptRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::ReadReceipt(req.receipt);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les annonces d'avatar (image de profil)
pub async fn run_sender_avatar(mut rx: Receiver<AvatarRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Avatar(req.announce);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les ACK de livraison
pub async fn run_sender_ack(mut rx: Receiver<MessageAckRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Ack(req.ack);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    use crate::message::{ChatMessage, NetworkPacket};

    #[tokio::test]
    async fn test_send_packet_delivers_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let packet = NetworkPacket::Chat(ChatMessage {
            from: "alice".to_string(),
            content: "test send".to_string(),
            timestamp: "10:00".to_string(),
            timestamp_epoch: None,
            to_user: Some("bob".to_string()),
        });
        let expected = serde_json::to_vec(&packet).unwrap();

        let recv = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).await.unwrap();
            buf
        });

        super::send_packet(addr, packet).await;

        let received = tokio::time::timeout(Duration::from_secs(2), recv)
            .await.unwrap().unwrap();
        assert_eq!(received, expected);
    }

    #[tokio::test]
    async fn test_send_packet_deserializes_correctly() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let packet = NetworkPacket::Chat(ChatMessage {
            from: "rudy".to_string(),
            content: "héllo 🎉".to_string(),
            timestamp: "14:30".to_string(),
            timestamp_epoch: Some(1_750_000_000),
            to_user: None,
        });

        let recv = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).await.unwrap();
            serde_json::from_slice::<NetworkPacket>(&buf).unwrap()
        });

        super::send_packet(addr, packet).await;

        let decoded = tokio::time::timeout(Duration::from_secs(2), recv)
            .await.unwrap().unwrap();

        match decoded {
            NetworkPacket::Chat(m) => {
                assert_eq!(m.from, "rudy");
                assert_eq!(m.content, "héllo 🎉");
                assert_eq!(m.timestamp_epoch, Some(1_750_000_000));
            }
            _ => panic!("Mauvais type de paquet reçu"),
        }
    }

    #[tokio::test]
    async fn test_send_packet_connection_refused_does_not_panic() {
        let addr: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
        let packet = NetworkPacket::Chat(ChatMessage {
            from: "alice".to_string(),
            content: "unreachable".to_string(),
            timestamp: "10:00".to_string(),
            timestamp_epoch: None,
            to_user: None,
        });
        super::send_packet(addr, packet).await;
    }
}
