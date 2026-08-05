//! Lancement automatique à l'ouverture de session (Launch Agent macOS,
//! registre Windows, `~/.config/autostart` Linux) via la crate `auto-launch`.
//!
//! Politique : activé par défaut au **premier lancement d'un build release**
//! (jamais en debug/`cargo run`), désactivable dans Paramètres → Général.
//! La préférence est persistée dans la table kv (`autostart`).

use auto_launch::AutoLaunchBuilder;

fn launcher() -> anyhow::Result<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe()?;
    let exe = exe.to_string_lossy().to_string();
    let mut builder = AutoLaunchBuilder::new();
    builder.set_app_name("Abcom").set_app_path(&exe);
    #[cfg(target_os = "macos")]
    builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
    builder
        .build()
        .map_err(|e| anyhow::anyhow!("autostart : {e}"))
}

/// Active/désactive le lancement au démarrage au niveau système.
pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
    let auto = launcher()?;
    if enabled {
        auto.enable().map_err(|e| anyhow::anyhow!("{e}"))?;
    } else {
        auto.disable().map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

/// Applique la politique de premier lancement : en release, si aucune
/// préférence n'existe encore, active l'autostart et renvoie la valeur
/// effective à persister. En debug, ne touche jamais au système.
pub fn init_default(existing_pref: Option<bool>) -> bool {
    if cfg!(debug_assertions) {
        return existing_pref.unwrap_or(false);
    }
    match existing_pref {
        Some(enabled) => enabled,
        None => match set_enabled(true) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("activation impossible : {e}");
                false
            }
        },
    }
}
