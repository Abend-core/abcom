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
