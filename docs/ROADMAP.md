# 🗺️ Roadmap Abcom - Features & Security

Plan de développement d'Abcom avec focus sur la sécurité et les features prioritaires.

---

## 🎯 Vision

Abcom doit évoluer d'une **app de chat LAN simple** vers une **plateforme sécurisée et privée** avec:
- ✅ Identité utilisateur (liée au compte machine)
- ✅ Authentification par passphrase
- ✅ Chiffrement bout-à-bout
- ✅ Respect des données privées (zéro cloud, zéro serveur)

---

## 📋 Phases de développement

### Phase 1: Sécurité de base (v0.1.0) 🔐

**Objectif**: Identifier les utilisateurs et protéger les communications.

#### 1.1 Authentification & Identité
- [ ] **Identity System**
  - ✅ Account bound to machine user (via `whoami`)
  - [ ] Generate random passphrase on first launch
  - [ ] Store passphrase hash (bcrypt/argon2) in `~/.local/share/abcom/identity.json`
  - [ ] Prompt for passphrase on next launches for verification
  - [ ] Option to reset/regenerate passphrase
  
**Pseudo-code:**
```rust
// First launch
if not identity_exists() {
    username = get_current_unix_user() // "alice"
    passphrase = generate_random_passphrase(24) // "rainbow-silver-42-quantum"
    store_identity(username, hash(passphrase))
    show_passphrase_once() // "⚠️ Save this: rainbow-silver-42-quantum"
}

// Subsequent launches
passphrase = prompt_user("Enter your passphrase:")
verify_passphrase(passphrase) // Must match stored hash
```

#### 1.2 Message Signing
- [ ] Sign messages with machine user identity
- [ ] Verify sender authenticity (prevent spoofing)
- [ ] Add `signed_by: "alice"` to each message
- [ ] Reject messages from unknown signers (optional whitelist)

**Data format:**
```json
{
  "from": "alice",
  "content": "Hello Bob",
  "timestamp": "14:30",
  "to_user": null,
  "signature": "8f9e2a1c...",
  "public_key": "-----BEGIN RSA PUBLIC KEY-----..."
}
```

#### 1.3 Key Exchange
- [ ] Generate RSA keypair per user on first launch
- [ ] Exchange public keys via UDP broadcast
- [ ] Store trusted public keys in `~/.local/share/abcom/trusted_keys/`
- [ ] Warn user about new/untrusted keys

---

### Phase 2: Encryption (v0.2.0) 🔒

**Objectif**: End-to-end encryption pour les messages.

#### 2.1 Transport Encryption
- [ ] TLS/SSL for TCP P2P connections
- [ ] Certificate pinning (self-signed certs)
- [ ] Fallback to unencrypted for broadcast (UDP multicast limitation)

#### 2.2 Message Encryption
- [ ] AES-256-GCM for per-message encryption
- [ ] Encrypt message content with recipient's public key (RSA-OAEP)
- [ ] Hybrid encryption: RSA for key exchange, AES for payload

**Message flow:**
```
Alice → Encrypt(content, bob_public_key) → Send encrypted
Bob → Decrypt(encrypted_content, bob_private_key) → Display
```

#### 2.3 Group Messages
- [ ] Encrypt with multiple recipients' keys
- [ ] Or use shared group key (requires key agreement protocol)

---

### Phase 3: User Management (v0.3.0) 👥

**Objectif**: Support pour multiples machines et identités.

- [ ] **Passphrase Strength Meter**
  - Display entropy score
  - Suggest stronger phrases
  
- [ ] **Passphrase Recovery**
  - Backup passphrase to file (encrypted)
  - Or recovery codes (like 2FA backup codes)
  
- [ ] **Device Management**
  - List devices (machines) connected
  - Revoke untrusted devices
  - See public key fingerprint per device
  
- [ ] **User Profiles**
  - Display name (different from username)
  - Avatar/emoji
  - Status (online/away/offline)

---

### Phase 4: Advanced Security (v0.4.0+) 🛡️

#### 4.1 Forward Secrecy
- [ ] Perfect Forward Secrecy (PFS) with ECDHE
- [ ] Session keys rotate regularly
- [ ] Old messages undecryptable if key leaked

#### 4.2 Anti-Tampering
- [ ] Message authenticity verification (HMAC)
- [ ] Detect modified messages
- [ ] Warn user about tampering attempts

#### 4.3 Privacy Features
- [ ] Disable message history locally (ephemeral mode)
- [ ] Auto-delete messages after X days
- [ ] Secure wipe (overwrite) at deletion
- [ ] Read receipts (optional per conversation)

#### 4.4 Network Security
- [ ] Detect man-in-the-middle attacks
- [ ] Rate limiting (prevent floods)
- [ ] Replay attack detection
- [ ] Randomize UDP broadcast timing (prevent timing analysis)

---

## 🔑 Security Considerations

### Current State (v0.0.1)
⚠️ **NOT PRODUCTION READY FOR SENSITIVE DATA**
- No authentication (anyone with hostname can impersonate)
- No encryption (messages in plaintext)
- No identity verification (messages not signed)

### v0.1.0 Improvements
✅ Bot resistance: Passphrases prevent random peers
✅ User verification: Messages signed with identity
✅ Key infrastructure: Public key exchange ready

### Still TODO
⚠️ Encryption at rest (files unencrypted)
⚠️ Encryption in transit (optional TLS only)
⚠️ No credential rotation
⚠️ No audit logs

---

## 📊 Data Security

### Files stored locally (at risk)
```
~/.local/share/abcom/
├── messages.json           # Plaintext messages 🚨
├── identity.json           # Passphrase hash 
├── keys/
│   ├── private.pem         # Private key (MUST be restricted)
│   └── public.pem
└── trusted_keys/
    └── alice_public.pem
```

### File Permissions
- `identity.json` → 600 (user only)
- `private.pem` → 600 (user only)
- `messages.json` → 600 (user only)
- `public.pem` → 644 (readable by others)

### Encryption at Rest (Future)
- [ ] Encrypt `messages.json` with passphrase
- [ ] Encrypt private key on disk
- [ ] Use libsodium/NaCl for key derivation

---

## 🎯 Implementation Priority

1. **MVP (v0.1.0)** - Authn/ID:
   - [x] Identity binding to machine user
   - [x] Passphrase generation and storage
   - [x] Message signing
   - [ ] Key exchange

2. **Core (v0.2.0)** - Encryption:
   - [ ] TLS for TCP
   - [ ] AES-GCM for messages
   - [ ] RSA key exchange

3. **Polish (v0.3.0)** - UX:
   - [ ] Device management UI
   - [ ] Profile management
   - [ ] Trust indicators

4. **Advanced (v0.4.0+)** - PFS & Audit:
   - [ ] Perfect forward secrecy
   - [ ] Tamper detection
   - [ ] Audit logs

---

## 💾 Architecture

### Passphrase Flow
```
First launch:
  1. OS user detected: "alice"
  2. Generate passphrase: "rainbow-silver-42-quantum"
  3. Hash passphrase: bcrypt("rainbow-silver-42-quantum")
  4. Store in ~/.local/share/abcom/identity.json
  5. Show passphrase ONCE to user (copy to clipboard)
  6. Generate RSA keypair
  7. Store private key encrypted with passphrase (PBKDF2)

Subsequent launches:
  1. Load identity.json
  2. Prompt user: "Enter passphrase:"
  3. Hash user input
  4. Verify against stored hash
  5. Use passphrase to derive key to decrypt RSA private key
  6. Initialize networking with authenticated identity
```

### Message Authentication
```
Message creation:
  1. Create message object {from, content, to_user, timestamp}
  2. Serialize to JSON
  3. Sign with private key (RSA-PSS signature)
  4. Add signature to message
  5. Add public key fingerprint (for verification)
  6. Send

Message reception:
  1. Receive message with signature
  2. Verify signature with sender's public key
  3. If signature valid: display "✓ alice" (green)
  4. If signature invalid: display "✗ UNKNOWN" (red) + warning
  5. If key untrusted: ask user "Trust this key?"
```

---

## 🧪 Testing Strategy

### Security Tests
- [ ] Passphrase hashing consistency
- [ ] Message signature verification
- [ ] Key exchange simulation
- [ ] Tampering detection
- [ ] Replay attack prevention

### Attack Scenarios
- [ ] Attacker guesses passphrase (brute force resistance)
- [ ] MITM intercepts messages (signature verification)
- [ ] Message modified in transit (integrity check)
- [ ] Spoofed sender (identity verification)

---

## 📚 References

- [OWASP: Passphrase Guidelines](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
- [NaCl (libsodium)](https://github.com/jedisct1/libsodium)
- [Rustls](https://github.com/rustls/rustls)
- [Perfect Forward Secrecy](https://en.wikipedia.org/wiki/Forward_secrecy)

---

## 🤝 Contributing

Pour contribuer sur la sécurité:
1. **Audit** - Review security-related PRs carefully
2. **Testing** - Add security test cases
3. **Documentation** - Document security assumptions
4. **Bug Reports** - Report security issues privately first

---

**Last Updated**: 2026-04-27  
**Version**: v0.0.1 (Security Roadmap)  
**Status**: In Planning
