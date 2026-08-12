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

/// Backend graphique concret utilisable par wgpu sous Windows (résultat de
/// `resolve_gpu_backend`).
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

/// Backend imposé par un fichier texte du répertoire de données (contenu
/// `dx12` ou `vulkan`, à créer ou modifier à la main) — il n'existe aucun
/// réglage dans l'interface : c'est un contournement de pilote graphique, pas
/// une préférence d'utilisateur ordinaire. Absent, vide ou illisible :
/// `None`, laisse `resolve_gpu_backend` choisir.
fn gpu_backend_choice() -> Option<GpuBackend> {
    std::fs::read_to_string(gpu_backend_choice_path())
        .ok()
        .and_then(|s| GpuBackend::parse(&s))
}

/// Efface le fichier de préférence, retour à l'automatique.
///
/// Appelé uniquement par `resolve_gpu_backend`, quand ce choix vient de faire
/// planter le lancement précédent : un fichier qui impose un backend cassé
/// planterait sinon à chaque lancement, sans aucun moyen de s'en sortir
/// puisque l'application ne s'ouvre jamais assez longtemps pour qu'on
/// corrige quoi que ce soit.
fn clear_gpu_backend_choice() {
    let path = gpu_backend_choice_path();
    if let Err(error) = std::fs::remove_file(&path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("préférence de backend graphique non effacée : {error}");
        }
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

fn last_incomplete_attempt() -> Option<GpuBackend> {
    std::fs::read_to_string(gpu_backend_attempt_path())
        .ok()
        .and_then(|s| GpuBackend::parse(&s))
}

/// Décision pure, testable sans toucher au disque : backend à retenir pour ce
/// lancement, et si le fichier de préférence doit être effacé parce qu'il
/// vient de faire planter le lancement précédent.
///
/// `preferred` est déjà résolu (contenu du fichier, ou D3D12 par défaut) ;
/// `forced` indique s'il vient du fichier — sans ça, l'effacer n'aurait pas
/// de sens. Le garde-fou porte sur `preferred`, pas sur sa provenance :
/// qu'un backend cassé soit celui qu'impose le fichier ou simplement le
/// défaut, il ne doit jamais être rejoué à l'identique juste après avoir fait
/// planter le processus.
fn decide_gpu_backend(
    preferred: GpuBackend,
    forced: bool,
    crashed_last_time: Option<GpuBackend>,
) -> (GpuBackend, bool) {
    if crashed_last_time != Some(preferred) {
        return (preferred, false);
    }
    (preferred.other(), forced)
}

/// Résout le backend à utiliser pour ce lancement.
///
/// Le backend se fixe par un fichier texte (`gpu_backend`, dans le répertoire
/// de données, contenu `dx12` ou `vulkan`) — pas de réglage dans l'interface.
/// Absent → D3D12, le plus fiable en pratique sous Windows (cf.
/// `low_power_wgpu`).
///
/// Garde-fou, quelle que soit l'origine du choix : si le backend qu'on
/// s'apprête à retenir est celui que le lancement précédent a laissé en plan
/// (marqueur non nettoyé, donc plantage ou kill), on bascule sur l'autre pour
/// CE lancement — et le fichier est effacé s'il en était la cause, pour ne
/// pas rejouer un choix cassé indéfiniment.
pub fn resolve_gpu_backend() -> GpuBackend {
    let file_choice = gpu_backend_choice();
    let preferred = file_choice.unwrap_or(GpuBackend::Dx12);
    let (backend, should_clear_file) =
        decide_gpu_backend(preferred, file_choice.is_some(), last_incomplete_attempt());

    if backend != preferred {
        tracing::warn!(
            backend_precedent = preferred.as_str(),
            backend_choisi = backend.as_str(),
            "lancement précédent interrompu avant sa fermeture propre : bascule automatique de backend graphique"
        );
    }
    if should_clear_file {
        tracing::warn!(
            fichier = %gpu_backend_choice_path().display(),
            "backend imposé par ce fichier : il vient de planter, le fichier est effacé pour éviter une boucle"
        );
        clear_gpu_backend_choice();
    }
    backend
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
    use super::{decide_gpu_backend, GpuBackend};

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

    #[test]
    fn decide_keeps_the_preferred_backend_when_nothing_crashed() {
        assert_eq!(
            decide_gpu_backend(GpuBackend::Dx12, false, None),
            (GpuBackend::Dx12, false)
        );
        assert_eq!(
            decide_gpu_backend(GpuBackend::Vulkan, true, None),
            (GpuBackend::Vulkan, false)
        );
    }

    #[test]
    fn decide_ignores_a_crash_on_a_different_backend() {
        // Le marqueur d'un plantage sur Vulkan ne doit rien changer si on
        // s'apprêtait de toute façon à essayer D3D12.
        assert_eq!(
            decide_gpu_backend(GpuBackend::Dx12, false, Some(GpuBackend::Vulkan)),
            (GpuBackend::Dx12, false)
        );
    }

    /// Régression : un backend imposé par le fichier qui vient de planter ne
    /// doit jamais être rejoué à l'identique. Sans ce garde-fou, un fichier
    /// qui impose un backend cassé plante à chaque lancement, sans aucun
    /// moyen de s'en sortir puisque l'application ne s'ouvre jamais assez
    /// longtemps pour qu'on le corrige — le fichier doit donc aussi être
    /// effacé, pas seulement contourné pour ce lancement.
    #[test]
    fn decide_overrides_a_forced_choice_that_just_crashed_and_clears_it() {
        assert_eq!(
            decide_gpu_backend(GpuBackend::Vulkan, true, Some(GpuBackend::Vulkan)),
            (GpuBackend::Dx12, true)
        );
    }

    /// Même bascule quand c'est le défaut (pas de fichier) qui vient de
    /// planter — mais rien à effacer, puisqu'il n'y avait pas de fichier.
    #[test]
    fn decide_overrides_a_crashing_default_without_touching_any_file() {
        assert_eq!(
            decide_gpu_backend(GpuBackend::Dx12, false, Some(GpuBackend::Dx12)),
            (GpuBackend::Vulkan, false)
        );
    }
}
