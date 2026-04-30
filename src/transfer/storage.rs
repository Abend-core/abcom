use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use walkdir::WalkDir;

use super::{PreparedEntry, PreparedTransfer, TransferEntry, TransferEntryKind, TransferManifest};

pub fn prepare_transfer(from: &str, to: &str, paths: &[PathBuf]) -> Result<PreparedTransfer> {
    if paths.is_empty() {
        bail!("no files or folders selected");
    }

    let transfer_id = format!(
        "{}-{}",
        sanitize_component(from),
        chrono::Utc::now().timestamp_millis()
    );

    let mut used_root_names = HashSet::new();
    let mut roots = Vec::new();
    let mut prepared_entries = Vec::new();
    let mut total_bytes = 0_u64;

    for original_path in paths {
        let canonical = original_path
            .canonicalize()
            .with_context(|| format!("unable to access {}", original_path.display()))?;
        let root_name = unique_root_name(root_display_name(&canonical), &mut used_root_names);
        roots.push(root_name.clone());

        let metadata = std::fs::metadata(&canonical)
            .with_context(|| format!("unable to stat {}", canonical.display()))?;

        if metadata.is_file() {
            total_bytes += metadata.len();
            prepared_entries.push(PreparedEntry {
                source_path: canonical.clone(),
                relative_path: root_name,
                kind: TransferEntryKind::File,
                size_bytes: metadata.len(),
            });
            continue;
        }

        if !metadata.is_dir() {
            bail!("unsupported transfer path {}", canonical.display());
        }

        prepared_entries.push(PreparedEntry {
            source_path: canonical.clone(),
            relative_path: root_name.clone(),
            kind: TransferEntryKind::Directory,
            size_bytes: 0,
        });

        for entry in WalkDir::new(&canonical).min_depth(1) {
            let entry = entry?;
            let entry_path = entry.path().to_path_buf();
            let relative_suffix = entry_path
                .strip_prefix(&canonical)
                .with_context(|| format!("unable to relativize {}", entry_path.display()))?;
            let relative_path = join_relative(&root_name, relative_suffix);
            let metadata = entry.metadata()?;
            let (kind, size_bytes) = if metadata.is_dir() {
                (TransferEntryKind::Directory, 0)
            } else if metadata.is_file() {
                (TransferEntryKind::File, metadata.len())
            } else {
                continue;
            };
            total_bytes += size_bytes;
            prepared_entries.push(PreparedEntry {
                source_path: entry_path,
                relative_path,
                kind,
                size_bytes,
            });
        }
    }

    let label = if roots.len() == 1 {
        roots[0].clone()
    } else {
        format!("{} items", roots.len())
    };

    let manifest = TransferManifest {
        transfer_id,
        from: from.to_string(),
        to: to.to_string(),
        label,
        item_count: prepared_entries.len(),
        total_bytes,
        entries: prepared_entries
            .iter()
            .map(|entry| TransferEntry {
                relative_path: entry.relative_path.clone(),
                kind: entry.kind.clone(),
                size_bytes: entry.size_bytes,
            })
            .collect(),
    };

    Ok(PreparedTransfer {
        manifest,
        entries: prepared_entries,
    })
}

pub fn prepare_receive_root(manifest: &TransferManifest) -> Result<PathBuf> {
    let base = dirs::download_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    let root = base
        .join("abcom-transfers")
        .join(sanitize_component(&manifest.from))
        .join(sanitize_component(&manifest.transfer_id));
    std::fs::create_dir_all(&root)
        .with_context(|| format!("unable to create {}", root.display()))?;
    Ok(root)
}

pub fn resolve_output_path(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let mut sanitized = PathBuf::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(value) => sanitized.push(value),
            Component::CurDir => {}
            _ => return Err(anyhow!("unsafe transfer path: {}", relative_path)),
        }
    }

    if sanitized.as_os_str().is_empty() {
        bail!("empty transfer path");
    }

    Ok(root.join(sanitized))
}

fn root_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| sanitize_component(&path.display().to_string()))
}

fn unique_root_name(candidate: String, used: &mut HashSet<String>) -> String {
    if used.insert(candidate.clone()) {
        return candidate;
    }

    let mut index = 2usize;
    loop {
        let next = format!("{}-{}", candidate, index);
        if used.insert(next.clone()) {
            return next;
        }
        index += 1;
    }
}

fn join_relative(root_name: &str, suffix: &Path) -> String {
    let mut parts = vec![root_name.to_string()];
    parts.extend(
        suffix
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                _ => None,
            }),
    );
    parts.join("/")
}

pub fn sanitize_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '_',
        })
        .collect::<String>();

    if sanitized.is_empty() {
        sanitized.push('_');
    }

    sanitized
}

#[cfg(test)]
mod tests {
    use super::{prepare_transfer, resolve_output_path};

    #[test]
    fn resolve_output_path_rejects_parent_escape() {
        let root = std::env::temp_dir();
        assert!(resolve_output_path(&root, "../evil.txt").is_err());
    }

    #[test]
    fn resolve_output_path_keeps_safe_relative_path() {
        let root = std::env::temp_dir();
        let resolved = resolve_output_path(&root, "folder/file.txt").unwrap();
        assert!(resolved.ends_with("folder/file.txt"));
    }

    #[test]
    fn prepare_transfer_preserves_directory_structure() {
        let base = std::env::temp_dir().join(format!(
            "abcom-transfer-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(base.join("docs/sub")).unwrap();
        std::fs::write(base.join("docs/sub/readme.txt"), b"hello").unwrap();

        let prepared = prepare_transfer("alice", "bob", &[base.join("docs")]).unwrap();

        assert!(prepared
            .manifest
            .entries
            .iter()
            .any(|entry| entry.relative_path == "docs" && entry.kind == super::TransferEntryKind::Directory));
        assert!(prepared
            .manifest
            .entries
            .iter()
            .any(|entry| entry.relative_path == "docs/sub/readme.txt" && entry.size_bytes == 5));

        let _ = std::fs::remove_dir_all(base);
    }
}