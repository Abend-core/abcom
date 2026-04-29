use serde::{Deserialize, Serialize};

use crate::transfer::model::{TransferConversation, TransferManifest};

pub const DATA_MAGIC: [u8; 4] = *b"ABTR";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferOffer {
    pub transfer_id: String,
    pub from: String,
    pub conversation: TransferConversation,
    pub manifest: TransferManifest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ControlMessage {
    Offer(TransferOffer),
    Accept {
        transfer_id: String,
    },
    Reject {
        transfer_id: String,
        reason: String,
    },
    Completed {
        transfer_id: String,
    },
    Failed {
        transfer_id: String,
        message: String,
    },
}