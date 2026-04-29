use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferConversation {
    Peer { username: String },
    Group { group_name: String },
}

impl TransferConversation {
    pub fn key(&self) -> String {
        match self {
            Self::Peer { username } => username.clone(),
            Self::Group { group_name } => format!("#{group_name}"),
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            Self::Peer { username } => username.clone(),
            Self::Group { group_name } => format!("#{group_name}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransferRecipient {
    pub username: String,
    pub addr: SocketAddr,
}

#[derive(Clone, Debug)]
pub struct OutgoingTransferRequest {
    pub from: String,
    pub conversation: TransferConversation,
    pub recipients: Vec<TransferRecipient>,
    pub selection: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransferEntryKind {
    Directory,
    File,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferEntry {
    pub relative_path: String,
    pub kind: TransferEntryKind,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferManifest {
    pub transfer_id: String,
    pub label: String,
    pub total_bytes: u64,
    pub total_files: usize,
    pub entries: Vec<TransferEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Outgoing,
    Incoming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Preparing,
    WaitingForPeer,
    Transferring,
    Completed,
    Failed,
}

#[derive(Clone, Debug)]
pub struct TransferRecord {
    pub id: String,
    pub conversation: TransferConversation,
    pub peer_username: String,
    pub peer_addr: SocketAddr,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub label: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub total_files: usize,
    pub transferred_files: usize,
    pub current_path: Option<String>,
    pub destination_root: Option<PathBuf>,
    pub error: Option<String>,
    pub updated_at: SystemTime,
}

impl TransferRecord {
    pub fn progress_ratio(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.transferred_bytes as f32 / self.total_bytes as f32).clamp(0.0, 1.0)
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.status, TransferStatus::Completed | TransferStatus::Failed)
    }
}

#[derive(Clone, Debug)]
pub enum TransferEvent {
    Upsert(TransferRecord),
}

#[derive(Clone, Debug, Default)]
pub struct TransferState {
    pub items: Vec<TransferRecord>,
}

impl TransferState {
    pub fn apply(&mut self, event: TransferEvent) {
        match event {
            TransferEvent::Upsert(record) => {
                if let Some(existing) = self.items.iter_mut().find(|item| item.id == record.id) {
                    *existing = record;
                } else {
                    self.items.push(record);
                }

                self.items.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
                if self.items.len() > 64 {
                    self.items.truncate(64);
                }
            }
        }
    }
}