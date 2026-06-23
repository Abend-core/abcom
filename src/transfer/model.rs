use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

pub const TRANSFER_BUFFER_SIZE: usize = 64 * 1024;
const MAX_HEADER_BYTES: usize = 1024 * 1024;

pub fn max_header_bytes() -> usize {
    MAX_HEADER_BYTES
}

#[derive(Clone, Debug)]
pub struct TransferRequest {
    pub from: String,
    pub recipient: String,
    pub to_addr: SocketAddr,
    pub paths: Vec<PathBuf>,
}

/// Proposition de réception d'un fichier, transmise à l'UI pour décision.
/// L'utilisateur accepte ou refuse via `decision_tx`.
pub struct TransferOffer {
    pub transfer_id: String,
    pub from: String,
    pub label: String,
    pub total_bytes: u64,
    pub item_count: usize,
    pub decision_tx: oneshot::Sender<TransferDecision>,
}

/// Réponse de l'utilisateur à une proposition de transfert.
#[derive(Debug)]
pub struct TransferDecision {
    pub accept: bool,
    /// Dossier de destination choisi (présent uniquement si accepté).
    pub dest_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferEntryKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferEntry {
    pub relative_path: String,
    pub kind: TransferEntryKind,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferManifest {
    pub transfer_id: String,
    pub from: String,
    pub to: String,
    pub label: String,
    pub item_count: usize,
    pub total_bytes: u64,
    pub entries: Vec<TransferEntry>,
}

#[derive(Clone, Debug)]
pub struct PreparedEntry {
    pub source_path: PathBuf,
    pub relative_path: String,
    pub kind: TransferEntryKind,
    pub size_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct PreparedTransfer {
    pub manifest: TransferManifest,
    pub entries: Vec<PreparedEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    /// Le destinataire a refusé le transfert.
    Rejected,
}

#[derive(Clone, Debug)]
pub struct TransferProgress {
    pub transfer_id: String,
    pub peer: String,
    pub label: String,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub current_path: Option<String>,
    pub detail: String,
}