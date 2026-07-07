#!/usr/bin/env python3
"""Seed de démonstration : remplit les instances locales 1 (alice), 2 (bob)
et 3 (carol) avec un jeu de données couvrant tous les concepts de l'app :

- « Tous » (diffusion), salon de groupe #projet, trois conversations privées ;
- markdown complet (titres, gras, listes, citations, liens, blocs de code) ;
- multiligne, message 100 % emoji, réponses (reply_to), réactions et
  multi-réactions ;
- message très long (> 4 000 caractères → replié « Afficher la suite ») ;
- texte tellement long qu'il est parti en pièce jointe .txt (concept
  « collage → fichier ») ;
- GIF, clip et sticker Klipy (kind gif + URL), image et fichier joints.

Usage : fermer toutes les instances abcom, puis
    python3 scripts/seed-demo.py

Les tables messages/reactions/groups/read_counts des trois instances sont
RÉINITIALISÉES (identité, clés épinglées et préférences kv conservées).
"""

import json
import platform
import shutil
import sqlite3
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

ALICE, BOB, CAROL = "alice", "bob", "carol"
INSTANCE = {ALICE: 1, BOB: 2, CAROL: 3}
GROUP_KEY = "#projet"

# URLs Klipy (WebP hd) déjà utilisées dans l'app : gif, clip et sticker
# partagent le même transport (kind "gif" + URL, aucun octet streamé).
GIF_URL = "https://static.klipy.com/ii/c3a19a0b747a76e98651f2b9a3cca5ff/25/96/myGRHbwT.webp"
CLIP_URL = "https://static.klipy.com/ii/c3a19a0b747a76e98651f2b9a3cca5ff/5f/42/bGJJMmFi.webp"
STICKER_URL = "https://static.klipy.com/ii/c3a19a0b747a76e98651f2b9a3cca5ff/25/96/myGRHbwT.webp"


def data_dir(instance: int) -> Path:
    if platform.system() == "Darwin":
        base = Path.home() / "Library" / "Application Support"
    else:
        base = Path.home() / ".local" / "share"
    return base / f"abcom-{instance}"


def fnv1a(key: bytes) -> int:
    h = 14695981039346656037
    for b in key:
        h ^= b
        h = (h * 1099511628211) % (1 << 64)
    return h


def signed(u64: int) -> int:
    return u64 - (1 << 64) if u64 >= (1 << 63) else u64


def message_hash(msg: dict) -> int:
    """Réplique exacte de AppState::message_hash (FNV-1a)."""
    media_id = msg["media"]["id"] if msg.get("media") else ""
    nonce = msg.get("nonce")
    nonce_part = f":{nonce}" if nonce is not None else ""
    key = (
        f"{msg['from']}:{msg.get('to_user') or 'broadcast'}:"
        f"{msg.get('timestamp_epoch') or 0}:{msg['content']}:{media_id}{nonce_part}"
    )
    return fnv1a(key.encode("utf-8"))


class Seed:
    """Accumule messages/réactions, calcule les hashs, écrit dans les DB."""

    def __init__(self) -> None:
        self.rows: list[tuple[dict, set[str]]] = []  # (message, destinataires)
        self.reactions: list[tuple[int, str, str, set[str]]] = []
        self._nonce = 1000
        # Points de départ : avant-hier, hier et aujourd'hui (séparateurs de date).
        now = int(time.time())
        self.clock = now - 2 * 86400 + 9 * 3600  # avant-hier ~9h

    def tick(self, seconds: int = 180) -> int:
        self.clock += seconds
        return self.clock

    def msg(self, sender, to, content, dbs, *, media=None, reply_to=None, gap=180):
        epoch = self.tick(gap)
        self._nonce += 1
        message = {
            "from": sender,
            "content": content,
            "timestamp": time.strftime("%H:%M", time.localtime(epoch)),
            "timestamp_epoch": epoch,
            "to_user": to,
            "media": media,
            "reply_to": reply_to,
            "nonce": self._nonce,
        }
        self.rows.append((message, set(dbs)))
        return message_hash(message)

    def react(self, target_hash, emoji, users, dbs):
        for user in users:
            self.reactions.append((target_hash, emoji, user, set(dbs)))

    def write(self, groups: dict, read_counts: dict) -> None:
        for who, instance in INSTANCE.items():
            db_path = data_dir(instance) / "abcom.db"
            if not db_path.exists():
                sys.exit(f"Base absente : {db_path} — lancer l'instance une fois d'abord.")
            con = sqlite3.connect(db_path)
            # Table rase : uniquement les données du seed subsistent. La table
            # `peers` (alias, avatars, clés TOFU d'anciens tests) est vidée aussi
            # — la découverte UDP la repeuple à chaud au prochain lancement. Les
            # préférences (`kv` : thème, sons, autostart) sont conservées.
            con.execute("DELETE FROM messages")
            con.execute("DELETE FROM reactions")
            con.execute("DELETE FROM groups")
            con.execute("DELETE FROM read_counts")
            con.execute("DELETE FROM peers")
            for message, dbs in self.rows:
                if who not in dbs:
                    continue
                con.execute(
                    "INSERT INTO messages (hash, from_user, to_user, content, timestamp,"
                    " ts_epoch, media, reply_to, nonce) VALUES (?,?,?,?,?,?,?,?,?)",
                    (
                        signed(message_hash(message)),
                        message["from"],
                        message["to_user"],
                        message["content"],
                        message["timestamp"],
                        message["timestamp_epoch"],
                        json.dumps(message["media"], ensure_ascii=False)
                        if message["media"]
                        else None,
                        signed(message["reply_to"]) if message["reply_to"] else None,
                        message["nonce"],
                    ),
                )
            for target_hash, emoji, user, dbs in self.reactions:
                if who not in dbs:
                    continue
                con.execute(
                    "INSERT OR IGNORE INTO reactions (message_hash, emoji, username)"
                    " VALUES (?,?,?)",
                    (signed(target_hash), emoji, user),
                )
            for name, data in groups.items():
                con.execute(
                    "INSERT INTO groups (name, data) VALUES (?,?)",
                    (name, json.dumps(data, ensure_ascii=False)),
                )
            for conv, count in read_counts.get(who, {}).items():
                con.execute(
                    "INSERT INTO read_counts (username, count) VALUES (?,?)",
                    (conv, count),
                )
            con.commit()
            con.close()


def reset_instances() -> None:
    """Repart d'un état propre : vide le cache `media/` de chaque instance
    (les fichiers du seed y sont réécrits ensuite). Les tables SQLite de contenu
    et la table `peers` sont réinitialisées dans `Seed.write`. À appeler AVANT
    toute copie de média du seed."""
    for instance in INSTANCE.values():
        media = data_dir(instance) / "media"
        if media.exists():
            shutil.rmtree(media)
        media.mkdir(parents=True, exist_ok=True)


def install_media(file_id: str, source: Path, dbs) -> None:
    """Copie un fichier média réel dans le cache media/ des participants."""
    for who in dbs:
        dest_dir = data_dir(INSTANCE[who]) / "media"
        dest_dir.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, dest_dir / file_id)


def write_long_txt() -> Path:
    """Texte trop long pour le composeur (> 100 000 caractères) : le concept
    « collage → pièce jointe .txt »."""
    path = Path("/tmp/abcom-seed-notes.txt")
    paragraphs = [
        f"Section {i} — compte-rendu détaillé de la réunion produit, avec les "
        f"décisions, les actions à mener et les responsables associés. "
        f"Ce paragraphe est volontairement verbeux pour dépasser le plafond "
        f"du composeur et illustrer la bascule automatique en fichier texte.\n"
        for i in range(1, 401)
    ]
    path.write_text("NOTES DE RÉUNION (collage trop long → .txt)\n\n" + "\n".join(paragraphs))
    return path


def main() -> None:
    reset_instances()  # cache media/ vidé avant de réécrire les médias du seed
    everyone = {ALICE, BOB, CAROL}
    ab = {ALICE, BOB}
    ac = {ALICE, CAROL}
    bc = {BOB, CAROL}
    s = Seed()

    # ── « Tous » (diffusion) : avant-hier ────────────────────────────────────
    s.msg(ALICE, None, "Salut tout le monde 👋 bienvenue sur abcom !", everyone)
    h_md = s.msg(
        ALICE,
        None,
        "# Démo markdown\n"
        "Du **gras**, de l'*italique* et du `code inline`.\n"
        "- une liste\n- avec plusieurs points\n"
        "1. et une liste\n2. numérotée\n"
        "> une citation importante\n"
        "Un lien : [le dépôt](https://github.com/rxdy/abcom)\n"
        "```rust\nfn main() {\n    println!(\"hello LAN\");\n}\n```",
        everyone,
    )
    s.react(h_md, "🔥", [BOB, CAROL], everyone)
    s.react(h_md, "👍", [BOB], everyone)
    h_multi = s.msg(
        BOB,
        None,
        "Message multiligne :\npremière ligne\ndeuxième ligne\ntroisième ligne",
        everyone,
    )
    s.react(h_multi, "👍", [ALICE, CAROL], everyone)
    s.msg(CAROL, None, "😂🎉", everyone)  # 100 % emoji → affiché en grand
    s.msg(
        CAROL,
        None,
        "Très bonne démo, merci ! J'ai tout suivi.",
        everyone,
        reply_to=h_md,  # réponse au message markdown
    )
    long_body = "\n".join(
        f"ligne {i:03d} — journal applicatif de test pour illustrer le repli "
        f"automatique des messages très longs dans le fil"
        for i in range(1, 101)
    )
    h_long = s.msg(BOB, None, "Dump de logs :\n" + long_body, everyone, gap=600)
    s.react(h_long, "😮", [ALICE, BOB, CAROL], everyone)  # multi-réaction 3 users
    s.msg(CAROL, None, "", everyone, media={
        "id": "seed-gif-tous",
        "filename": "gif.webp",
        "kind": "gif",
        "size_bytes": 0,
        "url": GIF_URL,
        "width": 300,
        "height": 224,
    })

    # ── Salon #projet : hier ─────────────────────────────────────────────────
    s.tick(86400 - 3 * 3600)  # saut au lendemain
    s.msg(ALICE, GROUP_KEY, "Bienvenue dans le salon **#projet** 🚀", everyone)
    h_plan = s.msg(
        ALICE,
        GROUP_KEY,
        "Plan de la semaine :\n- [ ] finaliser les accusés de lecture\n"
        "- [ ] seed de démo\n- [ ] audit qualité",
        everyone,
    )
    s.react(h_plan, "👍", [BOB, CAROL], everyone)
    s.react(h_plan, "🎉", [CAROL], everyone)
    s.msg(BOB, GROUP_KEY, "Je prends l'audit.", everyone, reply_to=h_plan)
    s.msg(CAROL, GROUP_KEY, "Et moi le seed 😊", everyone, reply_to=h_plan)
    group_log = "\n".join(
        f"[{i:04d}] trace d'exécution du pipeline CI, étape {i % 7}, statut OK"
        for i in range(1, 91)
    )
    s.msg(BOB, GROUP_KEY, "Sortie CI complète :\n" + group_log, everyone, gap=900)
    s.msg(BOB, GROUP_KEY, "", everyone, media={
        "id": "seed-clip-projet",
        "filename": "clip.webp",
        "kind": "gif",
        "size_bytes": 0,
        "url": CLIP_URL,
        "width": 400,
        "height": 268,
    })
    s.msg(CAROL, GROUP_KEY, "", everyone, media={
        "id": "seed-sticker-projet",
        "filename": "sticker.webp",
        "kind": "gif",
        "size_bytes": 0,
        "url": STICKER_URL,
        "width": 300,
        "height": 224,
    })

    # ── Privé alice ↔ bob : hier soir + image jointe ─────────────────────────
    s.tick(4 * 3600)
    s.msg(ALICE, BOB, "Salut Bob, tu as vu le nouveau logo ?", ab)
    icon = REPO / "assets" / "app_icon.png"
    image_id = "seed-logo-app_icon.png"
    install_media(image_id, icon, ab)
    h_img = s.msg(ALICE, BOB, "", ab, media={
        "id": image_id,
        "filename": "logo-abcom.png",
        "kind": "image",
        "size_bytes": icon.stat().st_size,
        "width": 500,
        "height": 500,
    })
    s.react(h_img, "❤️", [BOB], ab)
    s.msg(BOB, ALICE, "Très propre !\nOn le garde.", ab, reply_to=h_img)
    s.msg(ALICE, BOB, "Parfait 🎉", ab)

    # ── Privé alice ↔ carol : aujourd'hui + collage trop long → .txt ────────
    s.tick(86400 - 8 * 3600)  # saut à aujourd'hui matin
    s.msg(CAROL, ALICE, "Je t'envoie mes notes de réunion, c'était trop long "
                        "pour un message — parti en .txt automatiquement :", ac)
    txt = write_long_txt()
    txt_id = "seed-notes-reunion.txt"
    install_media(txt_id, txt, ac)
    s.msg(CAROL, ALICE, "", ac, media={
        "id": txt_id,
        "filename": "texte-colle-notes-reunion.txt",
        "kind": "file",
        "size_bytes": txt.stat().st_size,
    })
    s.msg(ALICE, CAROL, "Reçu, merci ! 📄", ac)

    # ── Privé bob ↔ carol : aujourd'hui + GIF ────────────────────────────────
    s.msg(BOB, CAROL, "Pause café ? ☕", bc)
    h_gif2 = s.msg(CAROL, BOB, "", bc, media={
        "id": "seed-gif-cafe",
        "filename": "gif.webp",
        "kind": "gif",
        "size_bytes": 0,
        "url": CLIP_URL,
        "width": 400,
        "height": 268,
    })
    s.react(h_gif2, "😂", [BOB], bc)
    s.msg(BOB, CAROL, "😂 on y va", bc)

    # ── Groupes et compteurs de lecture ──────────────────────────────────────
    created = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(s.clock - 86400))
    groups = {
        "projet": {
            "name": "projet",
            "owner": ALICE,
            "members": [ALICE, BOB, CAROL],
            "created_at": created,
        }
    }
    # Tout est lu, sauf : bob n'a pas lu les derniers messages de carol, et
    # carol n'a pas ouvert le salon récemment (badges non-lus visibles).
    read_counts = {
        ALICE: {BOB: 2, CAROL: 2, GROUP_KEY: 6},
        BOB: {ALICE: 2, CAROL: 0, GROUP_KEY: 6},
        CAROL: {ALICE: 2, BOB: 2, GROUP_KEY: 3},
    }

    s.write(groups, read_counts)
    total = len(s.rows)
    print(f"Instances remises à zéro (messages, réactions, groupes, read_counts,"
          f" peers, cache media/).")
    print(f"Seed écrit : {total} messages, {len(s.reactions)} réactions, "
          f"1 salon, 3 instances (alice, bob, carol).")


if __name__ == "__main__":
    main()
