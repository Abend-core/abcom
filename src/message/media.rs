use serde::{Deserialize, Serialize};

/// Nature d'un média attaché à un message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// Image affichée en vignette (cliquable pour l'agrandir).
    Image,
    /// Tout autre fichier, affiché en carte téléchargeable.
    File,
}

/// Média (image ou fichier) attaché à un [`super::ChatMessage`].
///
/// Sur le réseau, `data` porte les octets bruts du fichier. À la réception, ces
/// octets sont écrits dans le dossier `media/` puis retirés (`data = None`)
/// avant d'enregistrer le message dans l'historique, afin de garder
/// `messages.json` léger : seule la référence (`id`, `filename`, dimensions…)
/// y subsiste.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MediaAttachment {
    /// Identifiant unique servant aussi de nom de fichier en cache
    /// (extension d'origine conservée).
    pub id: String,
    /// Nom d'origine, utilisé à l'affichage et au téléchargement.
    pub filename: String,
    pub kind: MediaKind,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Octets bruts : présents sur le réseau, absents une fois en cache disque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<u8>>,
}

impl MediaAttachment {
    /// Vrai si le fichier porte une extension d'image prise en charge.
    pub fn is_image_filename(filename: &str) -> bool {
        matches!(
            extension_lower(filename).as_deref(),
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp")
        )
    }
}

/// Extension d'un nom de fichier en minuscules, sans le point.
pub fn extension_lower(filename: &str) -> Option<String> {
    filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| !ext.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{extension_lower, MediaAttachment, MediaKind};

    #[test]
    fn detects_image_extensions() {
        assert!(MediaAttachment::is_image_filename("photo.PNG"));
        assert!(MediaAttachment::is_image_filename("a.jpeg"));
        assert!(!MediaAttachment::is_image_filename("rapport.pdf"));
        assert!(!MediaAttachment::is_image_filename("sans_extension"));
    }

    #[test]
    fn extension_is_lowercased() {
        assert_eq!(extension_lower("Image.JpG").as_deref(), Some("jpg"));
        assert_eq!(extension_lower("archive.tar.gz").as_deref(), Some("gz"));
        assert_eq!(extension_lower("noext"), None);
    }

    #[test]
    fn media_round_trip_strips_nothing_by_serde() {
        let att = MediaAttachment {
            id: "abc.png".to_string(),
            filename: "abc.png".to_string(),
            kind: MediaKind::Image,
            size_bytes: 3,
            width: Some(10),
            height: Some(20),
            data: Some(vec![1, 2, 3]),
        };
        let json = serde_json::to_string(&att).unwrap();
        let back: MediaAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, MediaKind::Image);
        assert_eq!(back.data, Some(vec![1, 2, 3]));
        assert_eq!(back.width, Some(10));
    }

    #[test]
    fn media_without_data_omits_field() {
        let att = MediaAttachment {
            id: "f.bin".to_string(),
            filename: "f.bin".to_string(),
            kind: MediaKind::File,
            size_bytes: 0,
            width: None,
            height: None,
            data: None,
        };
        let json = serde_json::to_string(&att).unwrap();
        assert!(!json.contains("data"));
        assert!(!json.contains("width"));
    }
}
