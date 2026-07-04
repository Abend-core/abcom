//! Transport chiffré : handshake Noise XX puis flux de messages AEAD.
//!
//! - **Confidentialité / intégrité / anti-rejeu** : chaque frame est chiffrée
//!   ChaCha20-Poly1305 avec nonces gérés par Noise.
//! - **Authentification mutuelle** : les clés statiques X25519 sont échangées
//!   pendant le handshake (motif XX) ; l'appairage username↔clé est vérifié
//!   en TOFU (« trust on first use », cf. [`TrustStore`]).
//! - **Framing** : sur le fil, chaque frame = `u32 BE` (longueur du
//!   ciphertext, ≤ 65535 — taille max d'un message Noise) suivi du
//!   ciphertext. Un message logique (paquet JSON, chunk média) commence par
//!   sa longueur totale en clair *dans* la première frame chiffrée, et peut
//!   s'étendre sur plusieurs frames (les avatars dépassent 64 Ko).
//!
//! Après le handshake, chaque côté envoie un [`Hello`] (son username) : le
//! récepteur vérifie que la clé statique reçue correspond à celle épinglée
//! pour ce nom — sinon la connexion est refusée et l'UI est alertée.

use std::collections::HashMap;
use std::sync::Mutex;

use snow::{Builder, TransportState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::app::StorageCmd;
use crate::identity::{Identity, NOISE_PATTERN};

/// Taille maximale d'un message Noise (limite du protocole).
const MAX_NOISE_MESSAGE: usize = 65535;
/// Charge utile maximale par frame (tag AEAD de 16 octets déduit).
pub const MAX_CHUNK: usize = MAX_NOISE_MESSAGE - 16;
/// Taille maximale d'un message logique reconstitué (paquet JSON ou chunk
/// média) : borne les allocations d'un pair malveillant.
const MAX_LOGICAL_MESSAGE: usize = 8 * 1024 * 1024;

fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

// ── Framing bas niveau ───────────────────────────────────────────────────

async fn write_frame(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    debug_assert!(data.len() <= MAX_NOISE_MESSAGE);
    stream.write_u32(data.len() as u32).await?;
    stream.write_all(data).await?;
    Ok(())
}

async fn read_frame(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let len = stream.read_u32().await? as usize;
    if len == 0 || len > MAX_NOISE_MESSAGE {
        return Err(to_io("frame de taille invalide"));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

// ── Handshake ────────────────────────────────────────────────────────────

fn builder(identity: &Identity) -> Result<Builder<'_>, std::io::Error> {
    let params = NOISE_PATTERN.parse().map_err(to_io)?;
    Builder::new(params)
        .local_private_key(&identity.private)
        .map_err(to_io)
}

/// Handshake côté appelant. Renvoie le canal chiffré et la clé statique du
/// pair distant.
pub async fn handshake_initiator(
    stream: &mut TcpStream,
    identity: &Identity,
) -> std::io::Result<(TransportState, Vec<u8>)> {
    let mut hs = builder(identity)?.build_initiator().map_err(to_io)?;
    let mut buf = vec![0u8; MAX_NOISE_MESSAGE];

    // -> e
    let len = hs.write_message(&[], &mut buf).map_err(to_io)?;
    write_frame(stream, &buf[..len]).await?;
    // <- e, ee, s, es
    let msg = read_frame(stream).await?;
    let mut payload = vec![0u8; MAX_NOISE_MESSAGE];
    hs.read_message(&msg, &mut payload).map_err(to_io)?;
    // -> s, se
    let len = hs.write_message(&[], &mut buf).map_err(to_io)?;
    write_frame(stream, &buf[..len]).await?;

    let remote = hs
        .get_remote_static()
        .ok_or_else(|| to_io("clé statique distante absente"))?
        .to_vec();
    Ok((hs.into_transport_mode().map_err(to_io)?, remote))
}

/// Handshake côté serveur (miroir de [`handshake_initiator`]).
pub async fn handshake_responder(
    stream: &mut TcpStream,
    identity: &Identity,
) -> std::io::Result<(TransportState, Vec<u8>)> {
    let mut hs = builder(identity)?.build_responder().map_err(to_io)?;
    let mut buf = vec![0u8; MAX_NOISE_MESSAGE];
    let mut payload = vec![0u8; MAX_NOISE_MESSAGE];

    // <- e
    let msg = read_frame(stream).await?;
    hs.read_message(&msg, &mut payload).map_err(to_io)?;
    // -> e, ee, s, es
    let len = hs.write_message(&[], &mut buf).map_err(to_io)?;
    write_frame(stream, &buf[..len]).await?;
    // <- s, se
    let msg = read_frame(stream).await?;
    hs.read_message(&msg, &mut payload).map_err(to_io)?;

    let remote = hs
        .get_remote_static()
        .ok_or_else(|| to_io("clé statique distante absente"))?
        .to_vec();
    Ok((hs.into_transport_mode().map_err(to_io)?, remote))
}

// ── Canal chiffré ────────────────────────────────────────────────────────

/// Connexion TCP chiffrée : messages logiques de taille arbitraire (bornée),
/// découpés/reconstitués en frames Noise.
pub struct SecureStream {
    stream: TcpStream,
    transport: TransportState,
}

impl SecureStream {
    pub fn new(stream: TcpStream, transport: TransportState) -> Self {
        Self { stream, transport }
    }

    /// Envoie un message logique (chiffré, découpé en frames si nécessaire).
    pub async fn send(&mut self, plaintext: &[u8]) -> std::io::Result<()> {
        let mut ct = vec![0u8; MAX_NOISE_MESSAGE];
        // Première frame : longueur totale + début du contenu.
        let first_payload = plaintext.len().min(MAX_CHUNK - 4);
        let mut first = Vec::with_capacity(4 + first_payload);
        first.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
        first.extend_from_slice(&plaintext[..first_payload]);
        let len = self
            .transport
            .write_message(&first, &mut ct)
            .map_err(to_io)?;
        write_frame(&mut self.stream, &ct[..len]).await?;
        // Frames de continuation.
        for chunk in plaintext[first_payload..].chunks(MAX_CHUNK) {
            let len = self.transport.write_message(chunk, &mut ct).map_err(to_io)?;
            write_frame(&mut self.stream, &ct[..len]).await?;
        }
        self.stream.flush().await
    }

    /// Reçoit le prochain message logique (déchiffré, reconstitué).
    pub async fn recv(&mut self) -> std::io::Result<Vec<u8>> {
        let mut pt = vec![0u8; MAX_NOISE_MESSAGE];
        let frame = read_frame(&mut self.stream).await?;
        let n = self.transport.read_message(&frame, &mut pt).map_err(to_io)?;
        if n < 4 {
            return Err(to_io("en-tête de message manquant"));
        }
        let total = u32::from_be_bytes([pt[0], pt[1], pt[2], pt[3]]) as usize;
        if total > MAX_LOGICAL_MESSAGE {
            return Err(to_io("message logique trop volumineux"));
        }
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&pt[4..n]);
        while out.len() < total {
            let frame = read_frame(&mut self.stream).await?;
            let n = self.transport.read_message(&frame, &mut pt).map_err(to_io)?;
            out.extend_from_slice(&pt[..n]);
        }
        if out.len() != total {
            return Err(to_io("message logique incohérent"));
        }
        Ok(out)
    }
}

// ── Échange Hello (identification post-handshake) ────────────────────────

/// Envoie notre username puis lit celui du pair (les deux côtés le font,
/// dans un ordre imposé par le rôle pour éviter tout interblocage).
pub async fn exchange_hello(
    secure: &mut SecureStream,
    my_username: &str,
    initiator: bool,
) -> std::io::Result<String> {
    let hello = serde_json::to_vec(&crate::message::Hello {
        username: my_username.to_string(),
    })
    .map_err(to_io)?;
    if initiator {
        secure.send(&hello).await?;
        let reply = secure.recv().await?;
        let peer: crate::message::Hello = serde_json::from_slice(&reply).map_err(to_io)?;
        Ok(peer.username)
    } else {
        let first = secure.recv().await?;
        let peer: crate::message::Hello = serde_json::from_slice(&first).map_err(to_io)?;
        secure.send(&hello).await?;
        Ok(peer.username)
    }
}

// ── TOFU : épinglage username ↔ clé publique ─────────────────────────────

/// Verdict de vérification d'une clé de pair.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Trust {
    /// Première rencontre : la clé vient d'être épinglée.
    Pinned,
    /// La clé correspond à celle épinglée.
    Match,
    /// La clé NE correspond PAS : usurpation possible, connexion à refuser.
    Mismatch,
}

/// Magasin de confiance TOFU, partagé entre les tâches réseau. Les
/// épinglages sont persistés dans la table `peers` (SQLite).
pub struct TrustStore {
    keys: Mutex<HashMap<String, Vec<u8>>>,
    storage_tx: Option<std::sync::mpsc::Sender<StorageCmd>>,
}

impl TrustStore {
    pub fn new(
        keys: HashMap<String, Vec<u8>>,
        storage_tx: Option<std::sync::mpsc::Sender<StorageCmd>>,
    ) -> Self {
        Self {
            keys: Mutex::new(keys),
            storage_tx,
        }
    }

    /// Vérifie la clé d'un pair, l'épingle à la première rencontre.
    pub fn verify_and_pin(&self, username: &str, key: &[u8]) -> Trust {
        let mut keys = self.keys.lock().unwrap();
        match keys.get(username) {
            Some(pinned) if pinned == key => Trust::Match,
            Some(_) => Trust::Mismatch,
            None => {
                keys.insert(username.to_string(), key.to_vec());
                if let Some(tx) = &self.storage_tx {
                    let _ = tx.send(StorageCmd::UpsertPeerKey {
                        username: username.to_string(),
                        pubkey: key.to_vec(),
                    });
                }
                Trust::Pinned
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_network_secure.rs"]
mod tests;
