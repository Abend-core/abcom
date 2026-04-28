# 🗺️ Roadmap Abcom

Features & sécurité pour les prochaines versions.

---

## 🎯 Vision

Abcom: de **chat LAN simple** → **sécurisé & privé** (zéro cloud, données locales)

---

## 📋 Phases

### ✅ v0.0.1 (ACTUELLE)
- ✅ Chat P2P LAN
- ✅ Découverte auto pairs
- ✅ Interface native (egui)
- ✅ Messages persistants
- ✅ Indicateurs de frappe
- ✅ Notifications

---

### v0.1.0 - Authentification 🔐

**Focus**: Sécurité de base

**Features**:
- [ ] Account = utilisateur machine (whoami)
- [ ] Passphrase aléatoire à la première utilisation
- [ ] Stockage sécurisé de la passphrase
- [ ] Signature des messages (prouver l'identité)

**User**: Aucun compte externe, juste une passphrase locale

---

### v0.2.0 - Chiffrement 🔒

**Focus**: Messages chiffrés

**Features**:
- [ ] Chiffrement TCP (TLS/SSL)
- [ ] Chiffrement per-message
- [ ] Exchange de clés publiques

**User**: Tout communique de manière chiffrée

---

### v0.3.0 - User Management 👥

**Focus**: Confort d'utilisation

**Features**:
- [ ] Gestion de plusieurs appareils
- [ ] Profil utilisateur (avatar, status)
- [ ] Vérifier la passphrase avec reset/recover options

**User**: Partage facile entre machines

---

### v0.4.0+ - Advanced 🛡️

**Focus**: Confidentialité maximale

**Features**:
- [ ] Perfect Forward Secrecy (messages vieux indéchiffrables)
- [ ] Détection de tampering (message modifiés détectés)
- [ ] Messages éphémères (auto-delete)
- [ ] Logs d'audit

**User**: Privé ultra-sécurisé

---

## 📊 Timeline

- **v0.0.1** ✅ Done (27 avr 2026)
- **v0.1.0** 🔜 À faire (authentification)
- **v0.2.0** 🔜 Après (chiffrement)
- **v0.3.0** 🔜 Suite (user mgmt)
- **v0.4.0+** 🔜 Long term

---

## 🔑 Security Priorities

**Actuel (v0.0.1)** ⚠️ Non sécurisé:
- Pas d'authentification (anyone can impersonate)
- Messages en clair

**Priority 1** (v0.1.0): Prouver identité (signatures)  
**Priority 2** (v0.2.0): Chiffrer les messages  
**Priority 3** (v0.3.0): Confort & devices  
**Priority 4** (v0.4.0+): Ultra-privé  

---

**Questions?** Consulte README.md ou docs/
