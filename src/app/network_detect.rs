use crate::message::KnownNetwork;
use super::AppState;

impl AppState {
    /// Détecte le subnet /24 de l'interface principale
    pub fn detect_subnet() -> Option<String> {
        if let Ok(ip) = local_ip_address::local_ip() {
            if let std::net::IpAddr::V4(v4) = ip {
                let octs = v4.octets();
                if octs[0] != 127 {
                    return Some(format!("{}.{}.{}", octs[0], octs[1], octs[2]));
                }
            }
        }
        if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
            for (_name, ip) in &ifaces {
                if let std::net::IpAddr::V4(v4) = ip {
                    let octs = v4.octets();
                    if octs[0] != 127 && !(octs[0] == 169 && octs[1] == 254) {
                        return Some(format!("{}.{}.{}", octs[0], octs[1], octs[2]));
                    }
                }
            }
        }
        None
    }

    /// Détecte le SSID WiFi actuel
    pub fn detect_ssid() -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(out) = std::process::Command::new("iwgetid").arg("-r").output() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
            if let Ok(out) = std::process::Command::new("nmcli")
                .args(["-t", "-f", "active,ssid", "dev", "wifi"])
                .output()
            {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    if line.starts_with("yes:") {
                        let ssid = line[4..].to_string();
                        if !ssid.is_empty() {
                            return Some(ssid);
                        }
                    }
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(out) = std::process::Command::new("netsh")
                .args(["wlan", "show", "interfaces"])
                .output()
            {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let line = line.trim();
                    if line.starts_with("SSID") && !line.contains("BSSID") {
                        if let Some(ssid) = line.splitn(2, ':').nth(1) {
                            let ssid = ssid.trim().to_string();
                            if !ssid.is_empty() {
                                return Some(ssid);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Retourne (network_id, subnet)
    pub fn detect_network_id() -> (Option<String>, Option<String>) {
        let subnet = Self::detect_subnet();
        let ssid = Self::detect_ssid();
        let id = ssid.or_else(|| subnet.clone());
        (id, subnet)
    }

    /// Assure que le réseau est dans known_networks
    pub fn ensure_network_known(&mut self, id: &str, subnet: Option<&str>) {
        if !self.known_networks.iter().any(|n| n.id == id) {
            self.known_networks.push(KnownNetwork {
                id: id.to_string(),
                subnet: subnet.unwrap_or("").to_string(),
                alias: None,
                seen_peers: Vec::new(),
            });
            self.save_networks();
        }
    }
}
