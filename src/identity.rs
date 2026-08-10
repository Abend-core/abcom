//! Identité cryptographique locale : paire de clés X25519 statique utilisée
//! par le protocole Noise (cf. `network::secure`). Générée au premier
//! lancement, stockée dans le répertoire de données (permissions 0600).
//! La clé publique est l'identité vérifiable du pair (TOFU) ; son empreinte
//! est diffusée dans les annonces de découverte et affichable dans les
//! Paramètres.

use std::path::Path;

/// Motif Noise utilisé pour tout le transport : authentification mutuelle
/// par clés statiques échangées pendant le handshake + forward secrecy.
pub const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

#[derive(Clone)]
pub struct Identity {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

impl Identity {
    /// Charge la paire depuis `identity.key` (64 octets : privée ‖ publique),
    /// ou la génère et la persiste au premier lancement.
    pub fn load_or_create(base: &Path) -> anyhow::Result<Self> {
        let path = base.join("identity.key");
        if let Ok(bytes) = std::fs::read(&path) {
            if bytes.len() == 64 {
                // Une clé restaurée depuis une sauvegarde ou écrite par une
                // version antérieure peut porter des permissions larges : on
                // resserre à chaque chargement, pas seulement à la création.
                restrict_to_owner(&path);
                return Ok(Self {
                    private: bytes[..32].to_vec(),
                    public: bytes[32..].to_vec(),
                });
            }
            tracing::warn!("identity.key invalide, régénération");
        }

        let builder = snow::Builder::new(
            NOISE_PATTERN
                .parse()
                .map_err(|e| anyhow::anyhow!("motif Noise : {e}"))?,
        );
        let keypair = builder.generate_keypair()?;
        let mut bytes = keypair.private.clone();
        bytes.extend_from_slice(&keypair.public);
        std::fs::create_dir_all(base)?;
        write_private(&path, &bytes)?;
        tracing::info!(
            "nouvelle identité générée ({})",
            fingerprint(&keypair.public)
        );
        Ok(Self {
            private: keypair.private,
            public: keypair.public,
        })
    }

    /// Paire éphémère non persistée (tests, usages jetables).
    #[allow(dead_code)] // utilisé par les tests réseau et d'identité
    pub fn ephemeral() -> anyhow::Result<Self> {
        let builder = snow::Builder::new(
            NOISE_PATTERN
                .parse()
                .map_err(|e| anyhow::anyhow!("motif Noise : {e}"))?,
        );
        let keypair = builder.generate_keypair()?;
        Ok(Self {
            private: keypair.private,
            public: keypair.public,
        })
    }

    /// Empreinte courte de notre clé publique (affichage Paramètres).
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.public)
    }

    /// Clé publique en hexadécimal (annonce de découverte).
    pub fn public_hex(&self) -> String {
        hex(&self.public)
    }

    /// Clé de signature des annonces, dérivée de la clé Noise par BLAKE2s.
    ///
    /// Dérivée plutôt que stockée à part : `identity.key` garde son format, et
    /// la même identité produit toujours la même clé de signature. Le domaine
    /// évite toute réutilisation du secret entre les deux usages.
    pub fn signing_key(&self) -> ed25519_dalek::SigningKey {
        use blake2::{Blake2s256, Digest};
        let mut hasher = Blake2s256::new();
        hasher.update(b"abcom-discovery-signature-v1");
        hasher.update(&self.private);
        ed25519_dalek::SigningKey::from_bytes(&hasher.finalize().into())
    }

    /// Clé publique de vérification des annonces, en hexadécimal.
    pub fn verifying_hex(&self) -> String {
        hex(self.signing_key().verifying_key().as_bytes())
    }
}

/// Hexadécimal minuscule.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Écrit un secret dans un fichier créé d'emblée restreint à son propriétaire.
///
/// `std::fs::write` créerait le fichier selon l'umask (0644 le plus souvent)
/// et ne le resserrerait qu'ensuite : la clé privée serait lisible par tous
/// pendant cet intervalle. `mode` ne s'applique qu'à la création, d'où l'appel
/// à `restrict_to_owner` qui couvre la réécriture d'un fichier existant.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    restrict_to_owner(path);
    Ok(())
}

/// Réserve la clé à son propriétaire : 0600 sur Unix, réécriture d'ACL sur Windows où il est sans effet.
fn restrict_to_owner(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            tracing::warn!("permissions 0600 impossibles sur la clé : {error}");
        }
    }
    #[cfg(windows)]
    {
        let Ok(user) = std::env::var("USERNAME") else {
            tracing::warn!("ACL de la clé non restreinte : USERNAME absent");
            return;
        };
        let output = std::process::Command::new("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r"])
            .arg(format!("{user}:F"))
            .output();
        match output {
            Ok(out) if out.status.success() => {}
            Ok(out) => tracing::warn!(
                "ACL de la clé non restreinte : {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            Err(error) => tracing::warn!("ACL de la clé non restreinte : {error}"),
        }
    }
}

/// Empreinte lisible d'une clé publique : 8 groupes de 4 hexa.
pub fn fingerprint(public: &[u8]) -> String {
    let h = hex(public);
    h.as_bytes()
        .chunks(4)
        .take(8)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
#[path = "tests/test_identity.rs"]
mod tests;
