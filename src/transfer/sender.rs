use std::net::SocketAddr;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::protocol::DATA_MAGIC;
use super::storage::PreparedTransfer;

#[derive(Clone, Debug)]
pub struct SendTransferResult {
    pub bytes_sent: u64,
    pub files_sent: usize,
}

pub async fn send_transfer(
    prepared: &PreparedTransfer,
    recipient_addr: SocketAddr,
    mut on_progress: impl FnMut(u64, usize, Option<String>, bool),
) -> Result<SendTransferResult> {
    let mut stream = TcpStream::connect(recipient_addr)
        .await
        .with_context(|| format!("Connexion de données impossible vers {}", recipient_addr))?;

    let transfer_id_bytes = prepared.manifest.transfer_id.as_bytes();
    if transfer_id_bytes.len() > u16::MAX as usize {
        bail!("Identifiant de transfert trop long");
    }

    stream.write_all(&DATA_MAGIC).await?;
    stream.write_u16(transfer_id_bytes.len() as u16).await?;
    stream.write_all(transfer_id_bytes).await?;

    let mut total_sent = 0_u64;
    let mut files_sent = 0_usize;
    let mut buffer = vec![0_u8; 256 * 1024];

    for source in &prepared.sources {
        on_progress(total_sent, files_sent, Some(source.relative_path.clone()), false);

        let mut file = tokio::fs::File::open(&source.absolute_path)
            .await
            .with_context(|| format!("Impossible d'ouvrir {}", source.absolute_path.display()))?;
        let mut remaining = source.size;

        while remaining > 0 {
            let chunk_len = remaining.min(buffer.len() as u64) as usize;
            let read = file
                .read(&mut buffer[..chunk_len])
                .await
                .with_context(|| format!("Erreur de lecture sur {}", source.absolute_path.display()))?;
            if read == 0 {
                bail!("Lecture interrompue sur {}", source.absolute_path.display());
            }

            stream.write_all(&buffer[..read]).await?;
            total_sent += read as u64;
            remaining -= read as u64;
            on_progress(total_sent, files_sent, Some(source.relative_path.clone()), false);
        }

        files_sent += 1;
        on_progress(total_sent, files_sent, Some(source.relative_path.clone()), true);
    }

    stream.flush().await?;
    stream.shutdown().await?;

    Ok(SendTransferResult {
        bytes_sent: total_sent,
        files_sent,
    })
}