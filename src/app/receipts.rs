use std::net::SocketAddr;
use std::time::SystemTime;

use super::AppState;
use crate::message::{ChatMessage, SendRequest};

const MAX_RETRY_COUNT: u32 = 5;

/// Message en attente d'ACK
#[derive(Clone, Debug)]
pub struct PendingMessage {
    pub request: SendRequest,
    pub last_retry: SystemTime,
    pub retry_count: u32,
}

/// Liste nominative reçu/lu d'un message de groupe ou de « Tous »,
/// affichée par le popup « … » du fil (noms d'affichage, triés).
#[derive(Clone, Debug, Default)]
pub struct ReceiptDetail {
    pub delivered_by: Vec<String>,
    pub read_by: Vec<String>,
    /// Destinataires attendus, dénominateur du compteur « lu par n / N ».
    pub audience: usize,
}

impl AppState {
    /// Calcule un hash FNV-1a stable entre processus pour identifier les messages.
    ///
    /// Contrairement à DefaultHasher (graine aléatoire depuis Rust 1.36),
    /// FNV-1a produit le même résultat quel que soit le processus ou la machine,
    /// ce qui est indispensable : Alice calcule le hash côté envoi, Bob le recalcule
    /// côté réception pour l'inclure dans l'ACK — ils doivent tomber sur la même valeur.
    ///
    /// La clé inclut `timestamp_epoch` + `to_user` + `media.id` pour éviter les collisions
    /// entre messages identiques ou vides (cas des médias), plus le `nonce`
    /// quand il existe : deux messages au contenu identique envoyés dans la
    /// même seconde restent distincts. Le nonce n'est ajouté que s'il est
    /// présent, pour ne pas changer le hash des messages déjà persistés.
    pub fn message_hash(msg: &ChatMessage) -> u64 {
        msg.stable_hash()
    }

    pub fn mark_message_read(&mut self, message_hash: u64, username: String) {
        if self
            .read_receipts
            .entry(message_hash)
            .or_default()
            .insert(username.clone())
        {
            self.persist(super::StorageCmd::AddReceipt {
                hash: message_hash,
                username,
                kind: super::storage::ReceiptKind::Read,
            });
        }
        self.bump_content();
    }

    /// Enregistre un ACK de livraison nominatif reçu d'un pair (détail
    /// « reçu par » des salons et de « Tous »).
    pub fn mark_message_delivered_by(&mut self, message_hash: u64, username: String) {
        if self
            .delivered_receipts
            .entry(message_hash)
            .or_default()
            .insert(username.clone())
        {
            self.persist(super::StorageCmd::AddReceipt {
                hash: message_hash,
                username,
                kind: super::storage::ReceiptKind::Delivered,
            });
        }
        self.bump_content();
    }

    /// Adresses des pairs qui doivent recevoir un ACK/ReadReceipt concernant `msg`.
    ///
    /// - message privé → seulement l'expéditeur ;
    /// - salon (`#…`) → tous les membres en ligne (moi exclu) ;
    /// - diffusion (« Tous », `to_user == None`) → tous les pairs en ligne.
    ///
    /// Diffuser à tout le salon permet à chaque membre d'accumuler le même
    /// état reçu/lu et donc d'afficher le détail (« … ») sur chaque message.
    pub fn receipt_recipients(&self, msg: &ChatMessage) -> Vec<(String, SocketAddr)> {
        match msg.to_user.as_deref() {
            Some(target) if !target.starts_with('#') => self
                .peers
                .iter()
                .find(|p| p.username == msg.from && p.online && !p.addr.ip().is_unspecified())
                .map(|p| (p.username.clone(), p.addr))
                .into_iter()
                .collect(),
            Some(group_key) => group_key
                .strip_prefix('#')
                .map(|g| self.group_member_recipients(g))
                .unwrap_or_default(),
            None => self
                .peers
                .iter()
                .filter(|p| {
                    p.online && p.username != self.my_username && !p.addr.ip().is_unspecified()
                })
                .map(|p| (p.username.clone(), p.addr))
                .collect(),
        }
    }

    /// Destinataires attendus d'un message à plusieurs, moi exclu : membres du
    /// salon, ou pairs connus pour « Tous ».
    ///
    /// Compté sur les membres et non sur les pairs en ligne (contrairement à
    /// [`Self::receipt_recipients`], qui sert à l'envoi) : un dénominateur qui
    /// bouge à chaque déconnexion rendrait le compteur illisible.
    pub fn receipt_audience(&self, msg: &ChatMessage) -> usize {
        match msg.to_user.as_deref().and_then(|t| t.strip_prefix('#')) {
            Some(group_id) => self
                .get_group(group_id)
                .map(|g| g.members.iter().filter(|m| **m != self.my_username).count())
                .unwrap_or(0),
            None => self
                .peers
                .iter()
                .filter(|p| p.username != self.my_username)
                .count(),
        }
    }

    /// Liste nominative reçu/lu d'un message (noms d'affichage, triés),
    /// pour le détail des accusés des salons et de « Tous ».
    pub fn receipt_detail(&self, message_hash: u64, msg: &ChatMessage) -> ReceiptDetail {
        let resolve = |users: Option<&std::collections::HashSet<String>>| {
            let mut names: Vec<String> = users
                .map(|set| set.iter().map(|u| self.peer_display_name(u)).collect())
                .unwrap_or_default();
            names.sort();
            names
        };
        // Lire suppose avoir reçu. Un pair peut n'apparaître que dans les
        // accusés de lecture — c'était le cas des médias, dont la réception
        // n'émettait aucun ACK — et le fil affichait alors « lu par Bob, reçu
        // par personne », ce qui n'a aucun sens. On rétablit l'implication ici
        // pour que l'affichage reste cohérent quoi qu'il arrive en amont.
        let read = self.read_receipts.get(&message_hash);
        let mut delivered: std::collections::HashSet<String> = self
            .delivered_receipts
            .get(&message_hash)
            .cloned()
            .unwrap_or_default();
        if let Some(read) = read {
            delivered.extend(read.iter().cloned());
        }

        ReceiptDetail {
            delivered_by: resolve(Some(&delivered)),
            read_by: resolve(read),
            audience: self.receipt_audience(msg),
        }
    }

    pub fn get_read_count(&self, message_hash: u64) -> usize {
        self.read_receipts
            .get(&message_hash)
            .map(|r| r.len())
            .unwrap_or(0)
    }

    /// Met un message de côté jusqu'au retour en ligne du destinataire.
    pub fn queue_offline(&mut self, message: ChatMessage, to_peer: String) {
        let hash = Self::message_hash(&message);
        self.persist(super::StorageCmd::EnqueueOutbox {
            hash,
            to_peer: to_peer.clone(),
            message: message.clone(),
        });
        self.outbox.insert(hash, (to_peer, message));
        self.bump_content();
    }

    /// Messages en attente pour un pair qui revient en ligne, **sans** les
    /// retirer : ils ne sortent de la file qu'une fois la livraison confirmée,
    /// via [`Self::drop_from_outbox`]. Les vider plus tôt les perdrait si
    /// l'émission échouait ou si l'application fermait entre-temps.
    ///
    /// Ceux déjà confiés au réseau et en attente d'ACK sont écartés : leur
    /// réémission est du ressort du mécanisme de retry, pas de cette file.
    pub fn outbox_for(&self, peer: &str) -> Vec<(u64, ChatMessage)> {
        self.outbox
            .iter()
            .filter(|(hash, (to, _))| to == peer && !self.pending_messages.contains_key(hash))
            .map(|(hash, (_, message))| (*hash, message.clone()))
            .collect()
    }

    /// Retire un message de la file durable, une fois sa livraison **acquittée**.
    ///
    /// Jamais avant : l'admission dans le canal réseau ne garantit ni l'écriture
    /// sur la socket ni la réception. Un arrêt entre les deux laisserait le
    /// message uniquement en mémoire, donc sans réémission au redémarrage.
    pub fn drop_from_outbox(&mut self, message_hash: u64) {
        if self.outbox.remove(&message_hash).is_some() {
            self.persist(super::StorageCmd::DequeueOutbox { hash: message_hash });
        }
    }

    /// Le message attend-il encore le retour en ligne de son destinataire ?
    ///
    /// Un message confié au réseau et en attente d'ACK reste dans la file
    /// durable, mais n'est plus « en attente de reconnexion » pour l'affichage.
    pub fn is_queued_offline(&self, message_hash: u64) -> bool {
        self.outbox.contains_key(&message_hash)
            && !self.pending_messages.contains_key(&message_hash)
    }

    /// Marque un message comme envoyé (en attente d'ACK)
    pub fn mark_message_sent(&mut self, message_hash: u64, request: SendRequest) {
        self.failed_messages.remove(&message_hash);
        self.pending_messages.insert(
            message_hash,
            PendingMessage {
                request,
                last_retry: SystemTime::now(),
                retry_count: 0,
            },
        );
        self.bump_content();
    }

    pub fn mark_message_acked(&mut self, message_hash: u64, from_peer: &str) -> bool {
        let pending_matches = self
            .pending_messages
            .get(&message_hash)
            .is_some_and(|pending| pending.request.to_peer == from_peer);
        let failed_matches = self
            .failed_messages
            .get(&message_hash)
            .is_some_and(|peer| peer == from_peer);
        if pending_matches {
            self.pending_messages.remove(&message_hash);
        }
        if failed_matches {
            self.failed_messages.remove(&message_hash);
        }
        if pending_matches || failed_matches {
            // Livraison confirmée : c'est seulement ici que le message quitte
            // la file durable.
            self.drop_from_outbox(message_hash);
            self.bump_content();
            true
        } else {
            false
        }
    }

    /// Vérifie qu'un ACK provient bien d'un destinataire du message local.
    pub fn is_expected_ack_sender(&self, message_hash: u64, from_peer: &str) -> bool {
        let Some(message) = self
            .messages
            .iter()
            .find(|message| Self::message_hash(message) == message_hash)
        else {
            return false;
        };
        if message.from != self.my_username {
            return false;
        }
        match message.to_user.as_deref() {
            None => self.peers.iter().any(|peer| peer.username == from_peer),
            Some(group) if group.starts_with('#') => group
                .strip_prefix('#')
                .and_then(|name| self.get_group(name))
                .is_some_and(|group| group.members.iter().any(|member| member == from_peer)),
            Some(peer) => peer == from_peer,
        }
    }

    /// Les lectures privées ne concernent que l'auteur local. En salon et
    /// diffusion, chaque membre relaie le détail nominatif aux autres.
    pub fn is_expected_receipt_sender(&self, message_hash: u64, from_peer: &str) -> bool {
        let Some(message) = self
            .messages
            .iter()
            .find(|message| Self::message_hash(message) == message_hash)
        else {
            return false;
        };
        match message.to_user.as_deref() {
            None => self.peers.iter().any(|peer| peer.username == from_peer),
            Some(group) if group.starts_with('#') => group
                .strip_prefix('#')
                .and_then(|name| self.get_group(name))
                .is_some_and(|group| group.members.iter().any(|member| member == from_peer)),
            Some(peer) => message.from == self.my_username && peer == from_peer,
        }
    }

    /// Retourne les messages dus et bascule en échec ceux ayant épuisé leurs
    /// tentatives. Une tentative n'est comptée qu'après remise en file réussie.
    pub fn get_retry_messages(&mut self) -> (Vec<(u64, SendRequest)>, Vec<u64>) {
        let now = SystemTime::now();
        let mut to_retry = Vec::new();
        let mut failed = Vec::new();
        for (hash, pending) in &self.pending_messages {
            let delay = 2u64.saturating_pow(pending.retry_count.min(5));
            if let Ok(elapsed) = now.duration_since(pending.last_retry) {
                if elapsed.as_secs() >= delay {
                    if pending.retry_count >= MAX_RETRY_COUNT {
                        failed.push(*hash);
                    } else {
                        to_retry.push((*hash, pending.request.clone()));
                    }
                }
            }
        }
        for hash in &failed {
            if let Some(pending) = self.pending_messages.remove(hash) {
                self.failed_messages.insert(*hash, pending.request.to_peer);
            }
        }
        if !failed.is_empty() {
            self.bump_content();
        }
        (to_retry, failed)
    }

    pub fn mark_retry_enqueued(&mut self, message_hash: u64) {
        if let Some(pending) = self.pending_messages.get_mut(&message_hash) {
            pending.retry_count += 1;
            pending.last_retry = SystemTime::now();
        }
    }

    pub fn is_message_pending(&self, message_hash: u64) -> bool {
        self.pending_messages.contains_key(&message_hash)
    }

    pub fn is_message_failed(&self, message_hash: u64) -> bool {
        self.failed_messages.contains_key(&message_hash)
    }
}

#[cfg(test)]
#[path = "../tests/test_app_receipts.rs"]
mod tests;
