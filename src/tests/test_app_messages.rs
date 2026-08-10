use crate::app::AppState;
use crate::message::ChatMessage;

fn state(username: &str) -> AppState {
    let mut s = AppState::new(username.to_string(), Default::default(), None);
    s.messages.clear();
    s.peers.clear();
    s.read_marks.clear();
    s
}

fn msg(from: &str, to: Option<&str>, content: &str) -> ChatMessage {
    ChatMessage {
        from: from.to_string(),
        content: content.to_string(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: None,
        to_user: to.map(|s| s.to_string()),
        media: None,
        reply_to: None,
        nonce: None,
    }
}

#[test]
fn test_add_message_increases_count() {
    let mut s = state("alice");
    s.add_message(msg("bob", None, "hello"));
    assert_eq!(s.messages.len(), 1);
}

#[test]
fn test_unread_count_zero_no_messages() {
    let s = state("alice");
    assert_eq!(s.unread_count("bob"), 0);
}

#[test]
fn test_unread_count_increments() {
    let mut s = state("alice");
    s.messages.push(msg("bob", Some("alice"), "hi"));
    s.messages.push(msg("bob", Some("alice"), "hey"));
    assert_eq!(s.unread_count("bob"), 2);
}

#[test]
fn test_unread_count_zero_when_conversation_selected() {
    let mut s = state("alice");
    s.messages.push(msg("bob", Some("alice"), "hi"));
    s.selected_conversation = Some("bob".to_string());
    assert_eq!(s.unread_count("bob"), 0);
}

#[test]
fn test_mark_conversation_read_clears_unread() {
    let mut s = state("alice");
    s.messages.push(msg("bob", Some("alice"), "hi"));
    s.messages.push(msg("bob", Some("alice"), "hey"));
    s.mark_conversation_read("bob");
    assert_eq!(s.unread_count("bob"), 0);
}

#[test]
fn test_get_broadcast_messages() {
    let mut s = state("alice");
    s.messages.push(msg("bob", None, "broadcast"));
    s.messages.push(msg("bob", Some("alice"), "private"));
    // selected_conversation = None → broadcast only
    let result = s.get_conversation_messages();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "broadcast");
}

#[test]
fn test_get_private_conversation_messages() {
    let mut s = state("alice");
    s.messages.push(msg("bob", Some("alice"), "coucou"));
    s.messages.push(msg("alice", Some("bob"), "salut"));
    s.messages.push(msg("charlie", Some("alice"), "hey"));
    s.selected_conversation = Some("bob".to_string());
    let result = s.get_conversation_messages();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|m| m.from == "bob" || m.from == "alice"));
}

#[test]
fn test_clear_conversation_history_private() {
    let mut s = state("alice");
    s.messages.push(msg("bob", Some("alice"), "hi"));
    s.messages.push(msg("alice", Some("bob"), "ok"));
    s.messages.push(msg("charlie", Some("alice"), "hey"));
    s.selected_conversation = Some("bob".to_string());
    s.clear_conversation_history();
    // only charlie's message survives
    assert_eq!(s.messages.len(), 1);
    assert_eq!(s.messages[0].from, "charlie");
}

#[test]
fn test_clear_conversation_history_broadcast() {
    let mut s = state("alice");
    s.messages.push(msg("bob", None, "global"));
    s.messages.push(msg("bob", Some("alice"), "private"));
    // No selection → clear broadcast
    s.clear_conversation_history();
    assert_eq!(s.messages.len(), 1);
    assert_eq!(s.messages[0].to_user, Some("alice".to_string()));
}

#[test]
fn test_message_cap_at_500() {
    let mut s = state("alice");
    // Fill 500 messages then add 1 → drain 100 from front
    for i in 0..500 {
        s.messages.push(msg("bob", None, &i.to_string()));
    }
    s.add_message(msg("bob", None, "overflow"));
    assert_eq!(s.messages.len(), 401);
    assert_eq!(s.messages.last().unwrap().content, "overflow");
}

#[test]
fn unread_cache_follows_content_generation() {
    let dir = std::env::temp_dir().join(format!(
        "abcom-unread-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut s = AppState::new_with_base("me", &dir);

    let mut incoming = ChatMessage {
        from: "alice".into(),
        content: "1".into(),
        timestamp: "12:00".into(),
        timestamp_epoch: Some(1),
        to_user: Some("me".into()),
        media: None,
        reply_to: None,
        nonce: None,
    };
    s.add_message(incoming.clone());
    assert_eq!(s.unread_count("alice"), 1);

    // Le cache dérivé doit suivre chaque nouveau message…
    incoming.content = "2".into();
    incoming.timestamp_epoch = Some(2);
    s.add_message(incoming.clone());
    assert_eq!(s.unread_count("alice"), 2);

    // …nos propres messages ne comptent pas…
    s.add_message(ChatMessage {
        from: "me".into(),
        content: "reponse".into(),
        timestamp: "12:01".into(),
        timestamp_epoch: Some(3),
        to_user: Some("alice".into()),
        media: None,
        reply_to: None,
        nonce: None,
    });
    assert_eq!(s.unread_count("alice"), 2);

    // …et marquer lu remet le compteur à zéro.
    s.mark_conversation_read("alice");
    assert_eq!(s.unread_count("alice"), 0);
}

#[test]
fn duplicate_receptions_are_detected() {
    let dir = std::env::temp_dir().join(format!("abcom-dup-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = AppState::new_with_base("moi", &dir);

    let msg = ChatMessage {
        from: "alice".into(),
        content: "une seule fois".into(),
        timestamp: "12:00".into(),
        timestamp_epoch: Some(1),
        to_user: Some("moi".into()),
        media: None,
        reply_to: None,
        nonce: Some(42),
    };
    let hash = AppState::message_hash(&msg);

    assert!(!s.has_message(hash));
    s.add_message(msg.clone());
    // Réémission après un ACK perdu : le même message doit être reconnu, sinon
    // le retry en stockait jusqu'à six copies.
    assert!(s.has_message(hash));
    assert_eq!(s.messages.len(), 1);

    // Un message au contenu identique mais de nonce différent reste distinct.
    let mut other = msg.clone();
    other.nonce = Some(43);
    assert!(!s.has_message(AppState::message_hash(&other)));
}

#[test]
fn unread_survives_a_ring_buffer_purge() {
    let dir = std::env::temp_dir().join(format!("abcom-unread2-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = AppState::new_with_base("moi", &dir);

    let incoming = |n: u64| ChatMessage {
        from: "alice".into(),
        content: format!("m{n}"),
        timestamp: "12:00".into(),
        timestamp_epoch: Some(n),
        to_user: Some("moi".into()),
        media: None,
        reply_to: None,
        nonce: Some(n),
    };

    for n in 1..=5 {
        s.add_message(incoming(n));
    }
    assert_eq!(s.unread_count("alice"), 5);

    s.mark_conversation_read("alice");
    assert_eq!(s.unread_count("alice"), 0);

    s.add_message(incoming(6));
    s.add_message(incoming(7));
    assert_eq!(s.unread_count("alice"), 2);

    // Purge du début du fil : un compteur aurait désigné un autre ensemble et
    // affiché un décompte faux. Le repère par hash reste juste.
    s.messages.drain(0..3);
    assert_eq!(
        s.unread_count("alice"),
        2,
        "le repère de lecture doit survivre à la purge"
    );
}

#[test]
fn pagination_survives_a_window_overflow() {
    let dir = std::env::temp_dir().join(format!(
        "abcom-overflow-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = AppState::new_with_base("alice", &dir);
    // Position connue au départ : la pagination a de quoi repartir.
    s.oldest_loaded_rowid = Some(500);

    // Déborder la fenêtre mémoire.
    for i in 0..(s.history_cap() + 200) {
        s.add_message(msg("bob", None, &format!("m{i}")));
    }

    assert!(s.window_overflowed, "le débordement doit être mémorisé");
    assert_eq!(
        s.oldest_loaded_rowid, None,
        "les rowids sont inconnus après le drain"
    );
    // Le point clé : un rowid inconnu ne doit pas se confondre avec « plus
    // rien à charger ». Sans stockage branché la requête ne part pas, mais le
    // curseur doit être dérivable du plus ancien message encore en mémoire.
    assert!(
        !s.messages.is_empty(),
        "il reste des messages pour redériver le curseur"
    );

    // Une page rendue par le stockage réarme un curseur normal.
    s.prepend_older_messages(Vec::new(), Some(42));
    assert!(!s.window_overflowed);
    assert_eq!(s.oldest_loaded_rowid, Some(42));
}
