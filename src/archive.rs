//! Archivage des dossiers pour l'envoi en média.
//!
//! Un dossier est compressé en une archive ZIP unique (artefact ouvrable
//! proprement par le destinataire), ce qui permet de le traiter exactement
//! comme un fichier dans le chemin média.

use std::io::{Read, Write};
use std::path::Path;

use walkdir::WalkDir;

/// Compresse un dossier en archive ZIP directement dans un fichier (streamé,
/// sans charger l'archive en mémoire — adapté aux gros dossiers).
pub fn zip_dir_to_path(root: &Path, dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(dest)?;
    zip_dir_into(root, &mut file)
}

/// Écrit l'archive ZIP d'un dossier dans un flux quelconque. Les chemins internes
/// sont relatifs au dossier parent, de sorte que l'archive contienne le dossier
/// lui-même (`mon_dossier/...`).
fn zip_dir_into<W: Write + std::io::Seek>(root: &Path, writer: W) -> std::io::Result<()> {
    let base = root.parent().unwrap_or(root);
    let mut writer = zip::ZipWriter::new(writer);
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
    Ok(())
}

fn zip_to_io(err: zip::result::ZipError) -> std::io::Error {
    std::io::Error::other(err)
}

#[cfg(test)]
mod tests {
    use super::zip_dir_to_path;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("abcom_arch_{}_{}", label, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn zip_dir_to_path_contains_entries() {
        let dir = temp_dir("zip");
        let folder = dir.join("projet");
        std::fs::create_dir_all(folder.join("src")).unwrap();
        std::fs::write(folder.join("README.md"), b"# titre").unwrap();
        std::fs::write(folder.join("src/main.rs"), b"fn main() {}").unwrap();

        let archive_path = dir.join("projet.zip");
        zip_dir_to_path(&folder, &archive_path).unwrap();

        let file = std::fs::File::open(&archive_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "projet/README.md"));
        assert!(names.iter().any(|n| n == "projet/src/main.rs"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
