use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const TRANSFER_PORT: u16 = 9001;
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