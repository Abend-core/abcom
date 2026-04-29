use std::path::Path;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::model::{TransferEntryKind, TransferManifest};
use super::protocol::DATA_MAGIC;
use super::storage::resolve_output_path;

#[derive(Clone, Debug)]
pub struct ReceiveTransferResult {
    pub bytes_received: u64,
    pub files_received: usize,
}

pub async fn read_transfer_header(stream: &mut TcpStream) -> Result<String> {
    let mut magic = [0_u8; 4];
    stream.read_exact(&mut magic).await?;
    if magic != DATA_MAGIC {
        bail!("Flux de transfert invalide");
    }

    let transfer_id_len = stream.read_u16().await? as usize;
    if transfer_id_len == 0 {
        bail!("En-tête de transfert invalide");
    }

    let mut transfer_id = vec![0_u8; transfer_id_len];
    stream.read_exact(&mut transfer_id).await?;
    Ok(String::from_utf8(transfer_id).context("Identifiant de transfert invalide")?)
}

pub async fn receive_transfer(
    stream: &mut TcpStream,
    manifest: &TransferManifest,
    destination_root: &Path,
    mut on_progress: impl FnMut(u64, usize, Option<String>, bool),
) -> Result<ReceiveTransferResult> {
    let mut total_received = 0_u64;
    let mut files_received = 0_usize;
    let mut buffer = vec![0_u8; 256 * 1024];

    for entry in &manifest.entries {
        match entry.kind {
            TransferEntryKind::Directory => {
                let directory = resolve_output_path(destination_root, &entry.relative_path)?;
                tokio::fs::create_dir_all(&directory)
                    .await
                    .with_context(|| format!("Impossible de créer {}", directory.display()))?;
            }
            TransferEntryKind::File => {
                let output_path = resolve_output_path(destination_root, &entry.relative_path)?;
                if let Some(parent) = output_path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .with_context(|| format!("Impossible de créer {}", parent.display()))?;
                }

                let mut output = tokio::fs::File::create(&output_path)
                    .await
                    .with_context(|| format!("Impossible d'écrire {}", output_path.display()))?;
                let mut remaining = entry.size;

                on_progress(total_received, files_received, Some(entry.relative_path.clone()), false);

                while remaining > 0 {
                    let chunk_len = remaining.min(buffer.len() as u64) as usize;
                    let read = stream.read(&mut buffer[..chunk_len]).await?;
                    if read == 0 {
                        bail!("Connexion interrompue pendant la réception de {}", entry.relative_path);
                    }

                    output.write_all(&buffer[..read]).await?;
                    total_received += read as u64;
                    remaining -= read as u64;
                    on_progress(total_received, files_received, Some(entry.relative_path.clone()), false);
                }

                output.flush().await?;
                files_received += 1;
                on_progress(total_received, files_received, Some(entry.relative_path.clone()), true);
            }
        }
    }

    Ok(ReceiveTransferResult {
        bytes_received: total_received,
        files_received,
    })
}