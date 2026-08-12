//! Configuration runtime dérivée de l'environnement.
//!
//! Permet de lancer plusieurs instances de l'application sur la même machine
//! (tests P2P locaux) via la variable d'environnement `ABCOM_INSTANCE`.
//! Chaque instance utilise des ports TCP distincts et un répertoire de données
//! séparé, tout en partageant le port UDP de découverte (broadcast LAN) afin de
//! se voir mutuellement.
//!
//! Instance 0 (ou variable absente) = comportement de production inchangé :
//! chat TCP 9000, transfert TCP 9001, données dans `abcom`.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Port UDP de découverte — partagé par toutes les instances (broadcast LAN).
pub const DISCOVERY_PORT: u16 = 9001;

/// Identifiant d'instance, lu une seule fois depuis `ABCOM_INSTANCE` (défaut 0).
pub fn instance_id() -> u32 {
    static ID: OnceLock<u32> = OnceLock::new();
    *ID.get_or_init(|| {
        std::env::var("ABCOM_INSTANCE")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0)
    })
}

/// Nombre d'instances distinctes adressables sans sortir de la plage de ports.
///
/// Chaque instance consomme `chat_port` et `chat_port + 1` ; au-delà, le calcul
/// déborderait `u16` — en release il rebouclait silencieusement sur des ports
/// déjà attribués, en debug il paniquait.
const MAX_INSTANCE_ID: u32 = (u16::MAX as u32 - 9001) / 10;

/// Port TCP de chat de cette instance : 9000, 9010, 9020, …
pub fn chat_port() -> u16 {
    let id = instance_id().min(MAX_INSTANCE_ID);
    9000 + (id as u16) * 10
}

/// Port TCP de streaming des médias : toujours `chat_port + 1`.
pub fn media_port() -> u16 {
    chat_port() + 1
}

/// Clé API Klipy, lue une seule fois depuis `ABCOM_KLIPY_API_KEY`.
///
/// Renvoie `None` si la variable est absente ou vide : dans ce cas, le bouton
/// GIF affiche une notification au lieu d'ouvrir le sélecteur.
pub fn klipy_api_key() -> Option<String> {
    static KEY: OnceLock<Option<String>> = OnceLock::new();
    KEY.get_or_init(|| {
        std::env::var("ABCOM_KLIPY_API_KEY")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
    .clone()
}

/// Répertoire de données : `abcom` pour l'instance 0, `abcom-N` sinon.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    match instance_id() {
        0 => base.join("abcom"),
        n => base.join(format!("abcom-{n}")),
    }
}

/// Durée de vie des fichiers de travail avant purge automatique.
pub const SCRATCH_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Fichiers de travail (collages longs en `.txt`), en 0700 — pas `/tmp`, lisible par les autres comptes.
pub fn scratch_dir() -> std::io::Result<PathBuf> {
    let dir = data_dir().join("scratch");
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Backend graphique concret utilisable par wgpu sous Windows (choix
/// utilisateur ou résultat de `resolve_gpu_backend`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuBackend {
    Dx12,
    Vulkan,
}

impl GpuBackend {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "dx12" => Some(Self::Dx12),
            "vulkan" => Some(Self::Vulkan),
            _ => None,
        }
    }

    fn other(self) -> Self {
        match self {
            Self::Dx12 => Self::Vulkan,
            Self::Vulkan => Self::Dx12,
        }
    }
}

fn gpu_backend_choice_path() -> PathBuf {
    data_dir().join("gpu_backend")
}

/// Backend imposé depuis Paramètres → Général (Windows), ou `None` pour
/// laisser `resolve_gpu_backend` choisir.
pub fn gpu_backend_choice() -> Option<GpuBackend> {
    std::fs::read_to_string(gpu_backend_choice_path())
        .ok()
        .and_then(|s| GpuBackend::parse(&s))
}

/// Enregistre le choix (`None` efface la préférence, retour à l'automatique).
/// N'affecte que le prochain lancement : le backend est déjà figé pour la
/// session en cours.
pub fn set_gpu_backend_choice(choice: Option<GpuBackend>) {
    let path = gpu_backend_choice_path();
    let result = match choice {
        Some(backend) => std::fs::write(&path, backend.as_str()),
        None => std::fs::remove_file(&path).or_else(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(e),
        }),
    };
    if let Err(error) = result {
        tracing::warn!("préférence de backend graphique non sauvegardée : {error}");
    }
}

fn gpu_backend_attempt_path() -> PathBuf {
    data_dir().join("gpu_backend_attempt")
}

/// Marque le début d'une tentative avec `backend`, avant `eframe::run_native`.
/// À nettoyer avec `clear_gpu_backend_attempt` une fois la fenêtre refermée
/// proprement : un marqueur encore présent au lancement suivant signale que ce
/// backend a fait planter (ou tuer) le processus en cours de route.
pub fn mark_gpu_backend_attempt(backend: GpuBackend) {
    if let Err(error) = std::fs::write(gpu_backend_attempt_path(), backend.as_str()) {
        tracing::warn!("marqueur de backend graphique non écrit : {error}");
    }
}

pub fn clear_gpu_backend_attempt() {
    let _ = std::fs::remove_file(gpu_backend_attempt_path());
}

/// Résout le backend à utiliser pour ce lancement.
///
/// Priorité : préférence explicite de l'utilisateur ; sinon, si le lancement
/// précédent a laissé un marqueur (il ne s'est donc pas terminé proprement),
/// l'autre backend ; sinon D3D12, le plus fiable en pratique sous Windows —
/// voir le commentaire de `low_power_wgpu`.
pub fn resolve_gpu_backend() -> GpuBackend {
    if let Some(choice) = gpu_backend_choice() {
        return choice;
    }
    let Some(crashed) = std::fs::read_to_string(gpu_backend_attempt_path())
        .ok()
        .and_then(|s| GpuBackend::parse(&s))
    else {
        return GpuBackend::Dx12;
    };
    let fallback = crashed.other();
    tracing::warn!(
        backend_precedent = crashed.as_str(),
        backend_choisi = fallback.as_str(),
        "lancement précédent interrompu avant sa fermeture propre : bascule automatique de backend graphique"
    );
    fallback
}

/// Purge par ancienneté : le transfert média lit le fichier bien après la mise en file.
pub fn purge_scratch() {
    let Ok(dir) = scratch_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|modified| modified.elapsed().is_ok_and(|age| age > SCRATCH_TTL))
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GpuBackend;

    #[test]
    fn gpu_backend_round_trips_through_its_string_form() {
        assert_eq!(GpuBackend::parse("dx12"), Some(GpuBackend::Dx12));
        assert_eq!(GpuBackend::parse("vulkan"), Some(GpuBackend::Vulkan));
        assert_eq!(GpuBackend::parse("  dx12  "), Some(GpuBackend::Dx12));
        // Backend inconnu (fichier d'une version future, ou `gl` forcé via
        // `WGPU_BACKEND` un jour) : `None`, pas un panique ni un choix par défaut
        // silencieux qui masquerait le contenu réel du fichier.
        assert_eq!(GpuBackend::parse("gl"), None);
        assert_eq!(GpuBackend::parse(""), None);
    }

    /// L'alternance automatique ne doit jamais reproposer le backend qui vient
    /// de faire planter le processus : ce serait planter en boucle à chaque
    /// lancement au lieu de basculer.
    #[test]
    fn other_backend_never_returns_the_same_one() {
        assert_eq!(GpuBackend::Dx12.other(), GpuBackend::Vulkan);
        assert_eq!(GpuBackend::Vulkan.other(), GpuBackend::Dx12);
    }
}
