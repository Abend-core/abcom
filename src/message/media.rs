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
/// dans le dossier `media/<id>`. L'historique SQLite reste donc léger.
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

/// Hôtes dont un GIF distant peut être chargé.
///
/// Le champ `url` d'un média arrive d'un pair et `Image::from_uri` déclenche
/// une requête HTTP dès que le message devient visible — sans clic. Sans
/// filtre, n'importe quel pair transforme un message en balise de traçage :
/// l'hôte visé apprend l'adresse IP du destinataire et l'instant exact où il a
/// lu, hors de tout accusé de lecture, alors que le reste de l'application ne
/// sort jamais du réseau local.
///
/// Restreindre au CDN de Klipy ne déplace donc pas le problème : Klipy sert
/// déjà l'image à l'émetteur, il est dans la boucle par construction.
const ALLOWED_MEDIA_URL_HOSTS: &[&str] = &["klipy.com"];

/// Une URL de GIF est-elle chargeable sans risque ?
///
/// Exige `https` (pas de dégradation en clair) et un hôte de la liste, en
/// correspondance par suffixe de domaine — jamais par simple `contains`, qui
/// laisserait passer `klipy.com.pirate.example`.
pub fn media_url_is_loadable(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    // Port éventuel écarté avant comparaison.
    let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    ALLOWED_MEDIA_URL_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
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
    /// Indication de l'émetteur. Le destinataire la recalcule depuis la taille
    /// et le seuil du protocole avant de décider si un accord est nécessaire.
    pub requires_ack: bool,
}

/// Tâche d'envoi d'un média en streaming vers un destinataire (canal interne).
#[derive(Clone, Debug)]
pub struct MediaSendJob {
    /// Username attendu après le handshake Noise.
    pub to_peer: String,
    /// Adresse de chat du destinataire ; le port média vaut `+1`.
    pub to_addr: std::net::SocketAddr,
    /// Fichier source à streamer (fichier original ou archive d'un dossier).
    pub source_path: std::path::PathBuf,
    pub header: MediaStreamHeader,
}

/// Offre de réception d'un média volumineux (au-delà du seuil d'accord), transmise à l'UI pour
/// décision avant d'écrire le moindre octet.
pub struct MediaStreamOffer {
    pub from: String,
    pub to_user: Option<String>,
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
    /// En attente de l'acceptation du destinataire (émetteur, média au-delà du seuil d'accord).
    pub waiting: bool,
    /// Transfert terminé avec succès.
    pub finished: bool,
    /// Transfert interrompu (erreur réseau ou refus côté distant).
    pub failed: bool,
    /// `true` pour un transfert émis localement, `false` pour une réception.
    pub outgoing: bool,
}

/// Extension d'un nom de fichier en minuscules, sans le point.
pub fn extension_lower(filename: &str) -> Option<String> {
    filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| !ext.is_empty())
}

#[cfg(test)]
#[path = "../tests/test_message_media.rs"]
mod tests;
