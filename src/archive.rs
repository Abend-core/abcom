//! Archivage des dossiers pour l'envoi en média.
//!
//! Un dossier est compressé en une archive ZIP unique (artefact ouvrable
//! proprement par le destinataire), ce qui permet de le traiter exactement
//! comme un fichier dans le chemin média.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use walkdir::WalkDir;

/// Taille d'un chemin : longueur du fichier, ou somme récursive pour un dossier.
pub fn payload_size(path: &Path) -> u64 {
    if path.is_dir() {
        dir_size(path)
    } else {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    }
}

/// Somme récursive de la taille des fichiers d'un dossier.
pub fn dir_size(root: &Path) -> u64 {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Compresse un dossier en archive ZIP en mémoire. Les chemins internes sont
/// relatifs au dossier parent, de sorte que l'archive contienne le dossier
/// lui-même (`mon_dossier/...`).
pub fn zip_dir(root: &Path) -> std::io::Result<Vec<u8>> {
    let base = root.parent().unwrap_or(root);
    let mut buffer = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut buffer);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(base) else {
            continue;
        };
        let name = relative.to_string_lossy().replace('\\', "/");
        if name.is_empty() {
            continue;
        }

        if entry.file_type().is_dir() {
            writer
                .add_directory(format!("{name}/"), options)
                .map_err(zip_to_io)?;
        } else if entry.file_type().is_file() {
            writer.start_file(name, options).map_err(zip_to_io)?;
            let mut file = std::fs::File::open(path)?;
            let mut chunk = vec![0u8; 64 * 1024];
            loop {
                let read = file.read(&mut chunk)?;
                if read == 0 {
                    break;
                }
                writer.write_all(&chunk[..read])?;
            }
        }
    }

    writer.finish().map_err(zip_to_io)?;
    Ok(buffer.into_inner())
}

fn zip_to_io(err: zip::result::ZipError) -> std::io::Error {
    std::io::Error::other(err)
}

#[cfg(test)]
mod tests {
    use super::{dir_size, payload_size, zip_dir};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("abcom_arch_{}_{}", label, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dir_size_sums_files() {
        let dir = temp_dir("size");
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/b.txt"), b"world!").unwrap();
        assert_eq!(dir_size(&dir), 11);
        assert_eq!(payload_size(&dir), 11);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn payload_size_of_file() {
        let dir = temp_dir("fsize");
        let file = dir.join("f.bin");
        std::fs::write(&file, vec![0u8; 42]).unwrap();
        assert_eq!(payload_size(&file), 42);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn zip_dir_contains_entries() {
        let dir = temp_dir("zip");
        let folder = dir.join("projet");
        std::fs::create_dir_all(folder.join("src")).unwrap();
        std::fs::write(folder.join("README.md"), b"# titre").unwrap();
        std::fs::write(folder.join("src/main.rs"), b"fn main() {}").unwrap();

        let bytes = zip_dir(&folder).unwrap();
        let reader = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "projet/README.md"));
        assert!(names.iter().any(|n| n == "projet/src/main.rs"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
