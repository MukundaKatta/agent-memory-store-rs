/*!
agent-memory-store: simple key-value memory store for AI agents.

Persist facts, preferences, and summaries across conversation turns.
Supports tagging for grouped retrieval and tracks last-access time.

```rust
use agent_memory_store::MemoryStore;
use serde_json::json;

let mut store = MemoryStore::new();
store.store("user_name", json!("Alice"), &["user", "profile"]);
let entry = store.get("user_name").unwrap();
assert_eq!(entry.value, json!("Alice"));
assert_eq!(store.len(), 1);
```
*/

use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// A stored memory entry.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Value,
    pub tags: Vec<String>,
    pub created_at: f64,
    pub accessed_at: f64,
    pub access_count: usize,
}

impl MemoryEntry {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "key": self.key,
            "value": self.value,
            "tags": self.tags,
            "created_at": self.created_at,
            "accessed_at": self.accessed_at,
            "access_count": self.access_count,
        })
    }
}

/// Append-and-update key-value memory store.
#[derive(Debug, Default, Clone)]
pub struct MemoryStore {
    entries: HashMap<String, MemoryEntry>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `value` under `key`. Overwrites any existing entry.
    pub fn store(&mut self, key: &str, value: Value, tags: &[&str]) {
        let now = now_f64();
        let existing = self.entries.get(key);
        let created_at = existing.map(|e| e.created_at).unwrap_or(now);
        let access_count = existing.map(|e| e.access_count).unwrap_or(0);
        self.entries.insert(
            key.to_owned(),
            MemoryEntry {
                key: key.to_owned(),
                value,
                tags: tags.iter().map(|s| s.to_string()).collect(),
                created_at,
                accessed_at: now,
                access_count,
            },
        );
    }

    /// Get an entry by key, updating its access metadata.
    pub fn get(&mut self, key: &str) -> Option<&MemoryEntry> {
        let e = self.entries.get_mut(key)?;
        e.accessed_at = now_f64();
        e.access_count += 1;
        Some(e)
    }

    /// Get an entry by key without updating access metadata.
    pub fn peek(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.get(key)
    }

    /// Delete an entry. Returns `true` if the key existed.
    pub fn delete(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    /// All entries, sorted by key.
    pub fn all(&self) -> Vec<&MemoryEntry> {
        let mut v: Vec<&MemoryEntry> = self.entries.values().collect();
        v.sort_by_key(|e| e.key.as_str());
        v
    }

    /// Entries matching a specific tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Keys that currently exist.
    pub fn keys(&self) -> Vec<&str> {
        let mut k: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        k.sort();
        k
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Serialize all entries to a JSON array.
    pub fn to_json(&self) -> Value {
        Value::Array(self.all().iter().map(|e| e.to_json()).collect())
    }

    /// True if `key` exists.
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn store_and_peek() {
        let mut s = MemoryStore::new();
        s.store("k", json!("val"), &["tag1"]);
        let e = s.peek("k").unwrap();
        assert_eq!(e.value, json!("val"));
    }

    #[test]
    fn get_increments_access() {
        let mut s = MemoryStore::new();
        s.store("k", json!(1), &[]);
        s.get("k").unwrap();
        s.get("k").unwrap();
        assert_eq!(s.peek("k").unwrap().access_count, 2);
    }

    #[test]
    fn missing_key_returns_none() {
        let mut s = MemoryStore::new();
        assert!(s.get("missing").is_none());
        assert!(s.peek("missing").is_none());
    }

    #[test]
    fn overwrite_preserves_created_at() {
        let mut s = MemoryStore::new();
        s.store("k", json!(1), &[]);
        let t1 = s.peek("k").unwrap().created_at;
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.store("k", json!(2), &[]);
        let t2 = s.peek("k").unwrap().created_at;
        assert!((t1 - t2).abs() < 0.1);
    }

    #[test]
    fn delete_removes_entry() {
        let mut s = MemoryStore::new();
        s.store("k", json!(1), &[]);
        assert!(s.delete("k"));
        assert!(!s.contains("k"));
    }

    #[test]
    fn delete_returns_false_when_missing() {
        let mut s = MemoryStore::new();
        assert!(!s.delete("nope"));
    }

    #[test]
    fn by_tag_filters() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &["user"]);
        s.store("b", json!(2), &["user", "profile"]);
        s.store("c", json!(3), &["system"]);
        assert_eq!(s.by_tag("user").len(), 2);
        assert_eq!(s.by_tag("system").len(), 1);
        assert_eq!(s.by_tag("unknown").len(), 0);
    }

    #[test]
    fn all_sorted_by_key() {
        let mut s = MemoryStore::new();
        s.store("b", json!(2), &[]);
        s.store("a", json!(1), &[]);
        let all = s.all();
        assert_eq!(all[0].key, "a");
        assert_eq!(all[1].key, "b");
    }

    #[test]
    fn len_increments() {
        let mut s = MemoryStore::new();
        assert_eq!(s.len(), 0);
        s.store("k", json!(1), &[]);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn contains_returns_correct() {
        let mut s = MemoryStore::new();
        assert!(!s.contains("k"));
        s.store("k", json!(1), &[]);
        assert!(s.contains("k"));
    }

    #[test]
    fn clear_empties_store() {
        let mut s = MemoryStore::new();
        s.store("k", json!(1), &[]);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn to_json_is_array() {
        let mut s = MemoryStore::new();
        s.store("k", json!(1), &[]);
        let j = s.to_json();
        assert!(j.is_array());
        assert_eq!(j.as_array().unwrap().len(), 1);
    }

    #[test]
    fn entry_to_json_has_fields() {
        let mut s = MemoryStore::new();
        s.store("k", json!("v"), &["t"]);
        let e = s.peek("k").unwrap();
        let j = e.to_json();
        assert_eq!(j["key"], "k");
        assert_eq!(j["value"], "v");
        assert_eq!(j["tags"][0], "t");
    }

    #[test]
    fn keys_sorted() {
        let mut s = MemoryStore::new();
        s.store("c", json!(3), &[]);
        s.store("a", json!(1), &[]);
        s.store("b", json!(2), &[]);
        assert_eq!(s.keys(), vec!["a", "b", "c"]);
    }
}
