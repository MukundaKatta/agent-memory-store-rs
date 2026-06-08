//! Integration tests exercising the public API of `agent-memory-store`
//! the way a downstream crate would use it.

use agent_memory_store::MemoryStore;
use serde_json::json;

#[test]
fn end_to_end_session_lifecycle() {
    let mut store = MemoryStore::new();

    // An agent records a few facts over a conversation.
    store.store("user_name", json!("Alice"), &["user", "profile"]);
    store.store("user_city", json!("Paris"), &["user", "location"]);
    store.store("session_goal", json!("plan a trip"), &["session"]);

    assert_eq!(store.len(), 3);
    assert_eq!(store.keys(), vec!["session_goal", "user_city", "user_name"]);

    // Grouped retrieval by tag returns deterministic, key-sorted output.
    let user_facts = store.by_tag("user");
    assert_eq!(user_facts.len(), 2);
    assert_eq!(user_facts[0].key, "user_city");
    assert_eq!(user_facts[1].key, "user_name");

    // All distinct tags are discoverable.
    assert_eq!(store.tags(), vec!["location", "profile", "session", "user"]);

    // Reading bumps access metadata.
    let before = store.peek("user_name").unwrap().access_count;
    store.get("user_name");
    assert_eq!(store.peek("user_name").unwrap().access_count, before + 1);
}

#[test]
fn persistence_round_trip_via_string() {
    let mut store = MemoryStore::new();
    store.store("a", json!({"k": [1, 2, 3]}), &["t"]);
    store.store("b", json!("text"), &[]);
    store.get("a"); // create some access history to preserve

    // Serialize to a string, as a caller would when writing to a file.
    let serialized = store.to_json().to_string();

    // Parse it back and rebuild the store.
    let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    let restored = MemoryStore::from_json(&parsed).unwrap();

    assert_eq!(restored.len(), 2);
    assert_eq!(restored.peek("a").unwrap().value, json!({"k": [1, 2, 3]}));
    assert_eq!(restored.peek("a").unwrap().tags, vec!["t".to_string()]);
    assert_eq!(restored.peek("b").unwrap().value, json!("text"));
    // Access metadata survives the round-trip.
    assert!(restored.peek("a").unwrap().access_count >= 1);
}

#[test]
fn merging_two_snapshots() {
    let mut base = MemoryStore::new();
    base.store("shared", json!("old"), &[]);
    base.store("only_base", json!(1), &[]);

    let mut other = MemoryStore::new();
    other.store("shared", json!("new"), &[]);
    other.store("only_other", json!(2), &[]);

    let loaded = base.load_json(&other.to_json());

    assert_eq!(loaded, 2);
    assert_eq!(base.len(), 3);
    // Incoming snapshot wins on conflicting keys.
    assert_eq!(base.peek("shared").unwrap().value, json!("new"));
    assert!(base.contains("only_base"));
    assert!(base.contains("only_other"));
}

#[test]
fn bounded_memory_with_prune_lru() {
    let mut store = MemoryStore::new();
    for i in 0..10 {
        store.store(&format!("k{i}"), json!(i), &[]);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(store.len(), 10);

    // Keep only the 3 most recently accessed entries.
    let removed = store.prune_lru(3);
    assert_eq!(removed, 7);
    assert_eq!(store.len(), 3);
    // The most recently stored keys should survive.
    assert!(store.contains("k9"));
    assert!(store.contains("k8"));
    assert!(store.contains("k7"));
    assert!(!store.contains("k0"));
}
