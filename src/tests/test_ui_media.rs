
use super::{
    center_crop_uv, media_display_name, media_id, refused_media_message, unique_destination,
};
use eframe::egui;

#[test]
fn center_crop_uv_square_passthrough() {
    let (min, max) = center_crop_uv(egui::vec2(100.0, 100.0), 32.0, 32.0);
    assert_eq!(min, egui::pos2(0.0, 0.0));
    assert_eq!(max, egui::pos2(1.0, 1.0));
}

#[test]
fn center_crop_uv_portrait_crops_vertically() {
    // Image plus haute que large : on doit rogner haut/bas (marge en y).
    let (min, max) = center_crop_uv(egui::vec2(100.0, 200.0), 32.0, 32.0);
    assert_eq!(min.x, 0.0);
    assert_eq!(max.x, 1.0);
    assert!(min.y > 0.0 && min.y < 0.5);
    assert!((max.y - (1.0 - min.y)).abs() < 1e-6);
}

#[test]
fn center_crop_uv_landscape_crops_horizontally() {
    // Image plus large que haute : on doit rogner gauche/droite (marge en x).
    let (min, max) = center_crop_uv(egui::vec2(200.0, 100.0), 32.0, 32.0);
    assert_eq!(min.y, 0.0);
    assert_eq!(max.y, 1.0);
    assert!(min.x > 0.0 && min.x < 0.5);
    assert!((max.x - (1.0 - min.x)).abs() < 1e-6);
}

#[test]
fn display_name_of_file_and_folder() {
    let dir = std::env::temp_dir().join(format!("abcom_dn_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("rapport.pdf");
    std::fs::write(&file, b"x").unwrap();
    assert_eq!(media_display_name(&file), "rapport.pdf");
    assert_eq!(
        media_display_name(&dir),
        format!("{}.zip", dir.file_name().unwrap().to_str().unwrap())
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn media_id_sanitizes_and_keeps_extension() {
    let id = media_id("mon dossier/é@.png");
    assert!(id.contains('-'), "préfixe horodaté attendu");
    assert!(id.ends_with(".png"), "extension conservée");
    // Aucun caractère problématique de chemin n'est conservé.
    assert!(!id.contains('/') && !id.contains(' ') && !id.contains('@'));
}

#[test]
fn refused_message_is_attributed_to_sender() {
    let msg = refused_media_message("bob", "photo.zip", Some("ellis".to_string()));
    assert_eq!(msg.from, "bob");
    assert!(msg.content.contains("photo.zip"));
    assert!(msg.media.is_none());
    assert_eq!(msg.to_user.as_deref(), Some("ellis"));
}

#[test]
fn unique_destination_keeps_free_name() {
    let dir = std::env::temp_dir().join(format!("abcom_dl_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dest = unique_destination(&dir, "libre.txt");
    assert_eq!(dest, dir.join("libre.txt"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unique_destination_avoids_collision() {
    let dir = std::env::temp_dir().join(format!("abcom_dl2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("photo.png"), b"x").unwrap();
    let dest = unique_destination(&dir, "photo.png");
    assert_eq!(dest, dir.join("photo (1).png"));
    std::fs::remove_dir_all(&dir).ok();
}
