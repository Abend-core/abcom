use serde::{Deserialize, Serialize};

/// Nature d'un média attaché à un message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// Image affichée en vignette (cliquable pour l'agrandir).
    Image,
    /// GIF animé référencé par URL (Klipy) : aucun octet n'est streamé, chaque
    /// pair récupère l'animation depuis le CDN via le champ [`MediaAttachment::url`].
    Gif,
    /// Tout autre fichier, affiché en carte téléchargeable.
    File,
}

/// Référence d'un média (image ou fichier) attaché à un [`super::ChatMessage`].
///
/// Métadonnée uniquement : les octets ne transitent jamais dans le message, ils
/// sont transmis à part par streaming (voir `network::media_stream`) et écrits
/// dans le dossier `media/<id>`. L'historique `messages.json` reste donc léger.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MediaAttachment {
    /// Identifiant unique servant aussi de nom de fichier en cache
    /// (extension d'origine conservée).
    pub id: String,
    /// Nom d'origine, utilisé à l'affichage et au téléchargement.
    pub filename: String,
    pub kind: MediaKind,
    pub size_bytes: u64,
    /// URL source pour un GIF (variante WebP hd de Klipy). `None` pour les
    /// images et fichiers, dont les octets transitent par streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
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

/// En-tête envoyé en tête d'un flux média : il décrit le message et le média à
/// recevoir. Le destinataire reconstruit le [`super::ChatMessage`] à partir de
/// ces champs, puis reçoit les `media.size_bytes` octets qui suivent.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MediaStreamHeader {
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_user: Option<String>,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_epoch: Option<u64>,
    pub media: MediaAttachment,
    /// Vrai si le destinataire doit accepter avant la réception (> 1 Go).
    pub requires_ack: bool,
}

/// Tâche d'envoi d'un média en streaming vers un destinataire (canal interne).
#[derive(Clone, Debug)]
pub struct MediaSendJob {
    /// Adresse de chat du destinataire ; le port média vaut `+1`.
    pub to_addr: std::net::SocketAddr,
    /// Fichier source à streamer (fichier original ou archive d'un dossier).
    pub source_path: std::path::PathBuf,
    pub header: MediaStreamHeader,
}

/// Offre de réception d'un média volumineux (> 1 Go), transmise à l'UI pour
/// décision avant d'écrire le moindre octet.
pub struct MediaStreamOffer {
    pub from: String,
    pub filename: String,
    pub size_bytes: u64,
    pub decision_tx: tokio::sync::oneshot::Sender<bool>,
}

/// Progression d'un transfert média (émission ou réception), par identifiant.
#[derive(Clone, Debug)]
pub struct MediaProgress {
    pub id: String,
    pub done: u64,
    pub total: u64,
    /// En attente de l'acceptation du destinataire (émetteur, média > 1 Go).
    pub waiting: bool,
    /// Transfert terminé avec succès.
    pub finished: bool,
    /// Transfert interrompu (erreur réseau ou refus côté distant).
    pub failed: bool,
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
    fn media_round_trip() {
        let att = MediaAttachment {
            id: "abc.png".to_string(),
            filename: "abc.png".to_string(),
            kind: MediaKind::Image,
            size_bytes: 3,
            url: None,
            width: Some(10),
            height: Some(20),
        };
        let json = serde_json::to_string(&att).unwrap();
        let back: MediaAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, MediaKind::Image);
        assert_eq!(back.filename, "abc.png");
        assert_eq!(back.width, Some(10));
    }

    #[test]
    fn media_omits_absent_dimensions() {
        let att = MediaAttachment {
            id: "f.bin".to_string(),
            filename: "f.bin".to_string(),
            kind: MediaKind::File,
            size_bytes: 0,
            url: None,
            width: None,
            height: None,
        };
        let json = serde_json::to_string(&att).unwrap();
        assert!(!json.contains("width"));
    }

    #[test]
    fn gif_media_round_trip_keeps_url() {
        let att = MediaAttachment {
            id: "klipy-42".to_string(),
            filename: "gif.webp".to_string(),
            kind: MediaKind::Gif,
            size_bytes: 0,
            url: Some("https://cdn.klipy.com/hd.webp".to_string()),
            width: Some(480),
            height: Some(320),
        };
        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains("\"kind\":\"gif\""));
        let back: MediaAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, MediaKind::Gif);
        assert_eq!(back.url.as_deref(), Some("https://cdn.klipy.com/hd.webp"));
    }

    #[test]
    fn stream_header_round_trip() {
        let header = super::MediaStreamHeader {
            from: "bob".to_string(),
            to_user: Some("ellis".to_string()),
            timestamp: "12:00".to_string(),
            timestamp_epoch: Some(1_750_000_000),
            media: MediaAttachment {
                id: "x.zip".to_string(),
                filename: "x.zip".to_string(),
                kind: MediaKind::File,
                size_bytes: 6_000_000_000,
                url: None,
                width: None,
                height: None,
            },
            requires_ack: true,
        };
        let json = serde_json::to_string(&header).unwrap();
        let back: super::MediaStreamHeader = serde_json::from_str(&json).unwrap();
        assert_eq!(back.media.size_bytes, 6_000_000_000);
        assert!(back.requires_ack);
        assert_eq!(back.to_user.as_deref(), Some("ellis"));
    }
}
