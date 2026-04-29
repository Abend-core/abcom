use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use walkdir::WalkDir;

use crate::transfer::model::{TransferEntry, TransferEntryKind, TransferManifest};

#[derive(Clone, Debug)]
pub struct FileSource {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct PreparedTransfer {
    pub manifest: TransferManifest,
    pub sources: Vec<FileSource>,
}

pub fn prepare_transfer(selection: &[PathBuf]) -> anyhow::Result<PreparedTransfer> {
    if selection.is_empty() {
        bail!("Aucun fichier ou dossier sélectionné");
    }

    let mut top_level = HashSet::new();
    let mut entries = Vec::new();
    let mut sources = Vec::new();
    let mut total_bytes = 0_u64;
    let mut total_files = 0_usize;

    for path in selection {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("Impossible de lire les métadonnées de {}", path.display()))?;
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow!("Nom invalide pour {}", path.display()))?
            .to_string();

        if !top_level.insert(name.clone()) {
            bail!("Deux sélections partagent le même nom racine: {name}");
        }

        if metadata.is_dir() {
            entries.push(TransferEntry {
                relative_path: name.clone(),
                kind: TransferEntryKind::Directory,
                size: 0,
            });

            for walked in WalkDir::new(path).min_depth(1) {
                let walked = walked?;
                let child_path = walked.path();
                let relative = child_path
                    .strip_prefix(path)
                    .with_context(|| format!("Chemin relatif invalide pour {}", child_path.display()))?;
                let relative = Path::new(&name).join(relative);
                let relative_string = normalize_relative_path(&relative)?;
                let child_metadata = walked.metadata()?;

                if child_metadata.is_dir() {
                    entries.push(TransferEntry {
                        relative_path: relative_string,
                        kind: TransferEntryKind::Directory,
                        size: 0,
                    });
                    continue;
                }

                total_bytes += child_metadata.len();
                total_files += 1;
                entries.push(TransferEntry {
                    relative_path: relative_string.clone(),
                    kind: TransferEntryKind::File,
                    size: child_metadata.len(),
                });
                sources.push(FileSource {
                    relative_path: relative_string,
                    absolute_path: child_path.to_path_buf(),
                    size: child_metadata.len(),
                });
            }
            continue;
        }

        total_bytes += metadata.len();
        total_files += 1;
        entries.push(TransferEntry {
            relative_path: name.clone(),
            kind: TransferEntryKind::File,
            size: metadata.len(),
        });
        sources.push(FileSource {
            relative_path: name,
            absolute_path: path.clone(),
            size: metadata.len(),
        });
    }

    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    sources.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    let label = if selection.len() == 1 {
        selection[0]
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("élément")
            .to_string()
    } else {
        format!("{} éléments", selection.len())
    };

    Ok(PreparedTransfer {
        manifest: TransferManifest {
            transfer_id: String::new(),
            label,
            total_bytes,
            total_files,
            entries,
        },
        sources,
    })
}

pub fn ensure_receive_root(sender: &str, transfer_id: &str, label: &str) -> anyhow::Result<PathBuf> {
    let base = dirs::download_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("abcom")
        .join("received")
        .join(sanitize_path_component(sender));
    let short_id = transfer_id.split('-').take(2).collect::<Vec<_>>().join("-");
    let root = base.join(format!("{}_{}", sanitize_path_component(label), short_id));
    std::fs::create_dir_all(&root)
        .with_context(|| format!("Impossible de créer le dossier de réception {}", root.display()))?;
    Ok(root)
}

pub fn resolve_output_path(root: &Path, relative_path: &str) -> anyhow::Result<PathBuf> {
    let relative = Path::new(relative_path);
    validate_relative_path(relative)?;
    Ok(root.join(relative))
}

pub fn sanitize_path_component(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' ') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }

    let trimmed = output.trim_matches('.').trim().to_string();
    if trimmed.is_empty() {
        "transfer".to_string()
    } else {
        trimmed
    }
}

fn normalize_relative_path(path: &Path) -> anyhow::Result<String> {
    validate_relative_path(path)?;
    Ok(path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_string),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

fn validate_relative_path(path: &Path) -> anyhow::Result<()> {
    if path.is_absolute() {
        bail!("Chemin absolu interdit: {}", path.display());
    }

    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("Chemin non sûr: {}", path.display());
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!(
            "abcom-transfer-test-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn prepare_transfer_keeps_directory_structure() {
        let root = make_temp_dir("manifest");
        let folder = root.join("photos");
        let nested = folder.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("image.txt"), b"hello").unwrap();

        let prepared = prepare_transfer(&[folder.clone()]).unwrap();

        assert_eq!(prepared.manifest.total_files, 1);
        assert_eq!(prepared.manifest.total_bytes, 5);
        assert!(prepared.manifest.entries.iter().any(|entry| entry.relative_path == "photos"));
        assert!(prepared.manifest.entries.iter().any(|entry| entry.relative_path == "photos/nested"));
        assert!(prepared
            .manifest
            .entries
            .iter()
            .any(|entry| entry.relative_path == "photos/nested/image.txt"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_output_path_rejects_parent_components() {
        let root = make_temp_dir("path");
        let result = resolve_output_path(&root, "../secret.txt");
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}