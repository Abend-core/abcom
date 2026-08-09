use tokio::sync::mpsc;

use crate::app::{AppState, ConversationId, TransferTarget};
use crate::message::{ChatMessage, NetworkSendRequest, SendRequest};
use crate::util::MutexExt;

use super::AbcomApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueError {
    Full,
    Closed,
}

/// Réserve toutes les places avant d'en envoyer une seule. Une diffusion ne
/// peut ainsi pas être partiellement mise en file quand le canal est saturé.
fn queue_chat_requests(
    tx: &mpsc::Sender<NetworkSendRequest>,
    targets: &[TransferTarget],
    message: &ChatMessage,
) -> Result<Vec<SendRequest>, QueueError> {
    let requests: Vec<SendRequest> = targets
        .iter()
        .map(|target| SendRequest {
            to_peer: target.username.clone(),
            to_addr: target.addr,
            message: message.clone(),
        })
        .collect();
    let permits = tx
        .try_reserve_many(requests.len())
        .map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => QueueError::Full,
            mpsc::error::TrySendError::Closed(_) => QueueError::Closed,
        })?;
    for (permit, request) in permits.zip(requests.iter().cloned()) {
        permit.send(request.into());
    }
    Ok(requests)
}

impl AbcomApp {
    /// Point d'entrée unique pour les messages de chat, quel que soit leur
    /// contenu (texte, GIF ou futur type inline).
    pub(crate) fn enqueue_chat_message(&mut self, message: ChatMessage) -> bool {
        let (conversation, targets) = {
            let state = self.state.lock_safe();
            (
                state.selected_conversation_id(),
                state.selected_transfer_targets(),
            )
        };
        // Destinataire hors ligne : le message part dans le fil et dans la
        // file d'attente persistée, réémise à sa reconnexion.
        if let ConversationId::Peer(peer) = &conversation {
            if targets.is_empty() {
                let mut state = self.state.lock_safe();
                state.add_message(message.clone());
                state.queue_offline(message, peer.clone());
                drop(state);
                self.set_enqueue_error(
                    "Destinataire hors ligne : envoi à sa reconnexion",
                    "Recipient offline: will be sent when they reconnect",
                );
                return true;
            }
        }
        let requests = match queue_chat_requests(&self.net.send_tx, &targets, &message) {
            Ok(requests) => requests,
            Err(QueueError::Full) => {
                self.set_enqueue_error(
                    "File d'envoi pleine, message conservé",
                    "Send queue full, message kept",
                );
                return false;
            }
            Err(QueueError::Closed) => {
                self.set_enqueue_error(
                    "Réseau indisponible, message conservé",
                    "Network unavailable, message kept",
                );
                return false;
            }
        };

        let hash = AppState::message_hash(&message);
        let mut state = self.state.lock_safe();
        state.add_message(message);
        if matches!(conversation, ConversationId::Peer(_)) {
            if let Some(request) = requests.into_iter().next() {
                state.mark_message_sent(hash, request);
            }
        }
        true
    }

    fn set_enqueue_error(&mut self, french: &'static str, english: &'static str) {
        self.last_notification = Some(self.tr(french, english).to_string());
        self.notification_time = std::time::Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ChatMessage;

    fn message() -> ChatMessage {
        ChatMessage {
            from: "alice".into(),
            content: "hello".into(),
            timestamp: "12:00".into(),
            timestamp_epoch: None,
            to_user: None,
            media: None,
            reply_to: None,
            nonce: None,
        }
    }

    fn target(port: u16) -> TransferTarget {
        TransferTarget {
            username: format!("peer-{port}"),
            addr: format!("127.0.0.1:{port}").parse().unwrap(),
        }
    }

    #[test]
    fn reserves_a_whole_broadcast_before_sending() {
        let (tx, mut rx) = mpsc::channel(1);
        let error = queue_chat_requests(&tx, &[target(9001), target(9002)], &message());
        assert!(matches!(error, Err(QueueError::Full)));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn queues_every_reserved_target() {
        let (tx, mut rx) = mpsc::channel(2);
        let queued = queue_chat_requests(&tx, &[target(9001), target(9002)], &message()).unwrap();
        assert_eq!(queued.len(), 2);
        assert_eq!(rx.try_recv().unwrap().to_addr.port(), 9001);
        assert_eq!(rx.try_recv().unwrap().to_addr.port(), 9002);
    }
}
