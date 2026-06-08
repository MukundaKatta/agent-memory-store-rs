/*!
agent-memory-store: simple key-value memory store for AI agents.

Persist facts, preferences, and summaries across conversation turns.
Supports tagging for grouped retrieval and tracks last-access time.

# Quick start

```rust
use agent_memory_store::MemoryStore;
use serde_json::json;

let mut store = MemoryStore::new();
store.store("user_name", json!("Alice"), &["user", "profile"]);
let entry = store.get("user_name").unwrap();
assert_eq!(entry.value, json!("Alice"));
assert_eq!(store.len(), 1);
```

# Persistence round-trip

The store serializes to a plain JSON array with [`MemoryStore::to_json`] and
loads back with [`MemoryStore::from_json`], so you can save it to disk between
sessions and restore the full state — values, tags, and access metadata.

```rust
use agent_memory_store::MemoryStore;
use serde_json::json;

let mut store = MemoryStore::new();
store.store("city", json!("Paris"), &["travel"]);
store.store("lang", json!("rust"), &["pref"]);

// Serialize (e.g. write `snapshot.to_string()` to a file)...
let snapshot = store.to_json();

// ...then restore it later.
let restored = MemoryStore::from_json(&snapshot).unwrap();
assert_eq!(restored.len(), 2);
assert_eq!(restored.peek("city").unwrap().value, json!("Paris"));
```

# Bounding memory

Agents accumulate facts over long sessions. Cap the store with
[`MemoryStore::prune_lru`] to keep only the most recently accessed entries.

```rust
use agent_memory_store::MemoryStore;
use serde_json::json;

let mut store = MemoryStore::new();
store.store("a", json!(1), &[]);
store.store("b", json!(2), &[]);
store.store("c", json!(3), &[]);
store.get("c"); // touch `c` so it is most-recently accessed

let removed = store.prune_lru(2);
assert_eq!(removed, 1);
assert_eq!(store.len(), 2);
assert!(store.contains("c"));
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
    /// Serialize this entry to a JSON object.
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

    /// Reconstruct an entry from a JSON object produced by [`MemoryEntry::to_json`].
    ///
    /// Returns `None` if `value` is not a JSON object or is missing/has the
    /// wrong type for the required `key` field. Missing optional fields fall
    /// back to sensible defaults (empty tags, zeroed timestamps/counter), so
    /// the function is tolerant of partially-populated snapshots.
    pub fn from_json(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let key = obj.get("key")?.as_str()?.to_owned();
        let entry_value = obj.get("value").cloned().unwrap_or(Value::Null);
        let tags = obj
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let created_at = obj
            .get("created_at")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let accessed_at = obj
            .get("accessed_at")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let access_count = obj
            .get("access_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        Some(MemoryEntry {
            key,
            value: entry_value,
            tags,
            created_at,
            accessed_at,
            access_count,
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

    /// Entries matching a specific tag, sorted by key for deterministic output.
    pub fn by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        let mut v: Vec<&MemoryEntry> = self
            .entries
            .values()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect();
        v.sort_by_key(|e| e.key.as_str());
        v
    }

    /// All distinct tags across every entry, sorted alphabetically.
    pub fn tags(&self) -> Vec<String> {
        let mut set: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for e in self.entries.values() {
            for t in &e.tags {
                set.insert(t.as_str());
            }
        }
        set.into_iter().map(|s| s.to_owned()).collect()
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

    /// Build a store from a JSON array produced by [`MemoryStore::to_json`].
    ///
    /// Returns `None` if `value` is not a JSON array. Individual elements that
    /// cannot be parsed into a [`MemoryEntry`] (e.g. missing `key`) are skipped
    /// rather than failing the whole load. If the snapshot contains duplicate
    /// keys, the last occurrence wins.
    pub fn from_json(value: &Value) -> Option<Self> {
        let arr = value.as_array()?;
        let mut store = MemoryStore::new();
        for item in arr {
            if let Some(entry) = MemoryEntry::from_json(item) {
                store.entries.insert(entry.key.clone(), entry);
            }
        }
        Some(store)
    }

    /// Merge entries from a JSON array (as produced by [`MemoryStore::to_json`])
    /// into this store, overwriting any keys that already exist.
    ///
    /// Returns the number of entries successfully loaded. Returns `0` if
    /// `value` is not a JSON array.
    pub fn load_json(&mut self, value: &Value) -> usize {
        let Some(arr) = value.as_array() else {
            return 0;
        };
        let mut loaded = 0;
        for item in arr {
            if let Some(entry) = MemoryEntry::from_json(item) {
                self.entries.insert(entry.key.clone(), entry);
                loaded += 1;
            }
        }
        loaded
    }

    /// Evict least-recently-accessed entries until at most `max_entries` remain.
    ///
    /// Entries are ranked by `accessed_at` (oldest first); ties break by key for
    /// deterministic eviction. Returns the number of entries removed. A
    /// `max_entries` of `0` clears the store.
    pub fn prune_lru(&mut self, max_entries: usize) -> usize {
        if self.entries.len() <= max_entries {
            return 0;
        }
        let mut ranked: Vec<(f64, String)> = self
            .entries
            .values()
            .map(|e| (e.accessed_at, e.key.clone()))
            .collect();
        // Most-recently-accessed first so we can keep the head and drop the tail.
        ranked.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        let mut removed = 0;
        for (_, key) in ranked.into_iter().skip(max_entries) {
            if self.entries.remove(&key).is_some() {
                removed += 1;
            }
        }
        removed
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

    #[test]
    fn by_tag_sorted_by_key() {
        let mut s = MemoryStore::new();
        s.store("z", json!(1), &["t"]);
        s.store("a", json!(2), &["t"]);
        s.store("m", json!(3), &["t"]);
        let got: Vec<&str> = s.by_tag("t").iter().map(|e| e.key.as_str()).collect();
        assert_eq!(got, vec!["a", "m", "z"]);
    }

    #[test]
    fn tags_are_distinct_and_sorted() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &["user", "profile"]);
        s.store("b", json!(2), &["user", "system"]);
        assert_eq!(s.tags(), vec!["profile", "system", "user"]);
    }

    #[test]
    fn tags_empty_when_no_tags() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &[]);
        assert!(s.tags().is_empty());
    }

    #[test]
    fn entry_from_json_round_trip() {
        let mut s = MemoryStore::new();
        s.store("k", json!({"nested": [1, 2, 3]}), &["x", "y"]);
        s.get("k"); // bump access_count to 1
        let original = s.peek("k").unwrap().clone();
        let restored = MemoryEntry::from_json(&original.to_json()).unwrap();
        assert_eq!(restored.key, original.key);
        assert_eq!(restored.value, original.value);
        assert_eq!(restored.tags, original.tags);
        assert_eq!(restored.access_count, original.access_count);
        assert_eq!(restored.created_at, original.created_at);
        assert_eq!(restored.accessed_at, original.accessed_at);
    }

    #[test]
    fn entry_from_json_requires_key() {
        assert!(MemoryEntry::from_json(&json!({"value": 1})).is_none());
        assert!(MemoryEntry::from_json(&json!("not an object")).is_none());
    }

    #[test]
    fn entry_from_json_tolerates_missing_optional_fields() {
        let e = MemoryEntry::from_json(&json!({"key": "k"})).unwrap();
        assert_eq!(e.key, "k");
        assert_eq!(e.value, Value::Null);
        assert!(e.tags.is_empty());
        assert_eq!(e.access_count, 0);
        assert_eq!(e.created_at, 0.0);
    }

    #[test]
    fn store_from_json_round_trip() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &["t1"]);
        s.store("b", json!("two"), &["t2"]);
        let restored = MemoryStore::from_json(&s.to_json()).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.peek("a").unwrap().value, json!(1));
        assert_eq!(restored.peek("b").unwrap().value, json!("two"));
        assert_eq!(restored.by_tag("t1").len(), 1);
    }

    #[test]
    fn store_from_json_rejects_non_array() {
        assert!(MemoryStore::from_json(&json!({"key": "k"})).is_none());
    }

    #[test]
    fn store_from_json_skips_bad_entries() {
        let input = json!([
            {"key": "ok", "value": 1},
            {"value": "no key"},
            "garbage"
        ]);
        let restored = MemoryStore::from_json(&input).unwrap();
        assert_eq!(restored.len(), 1);
        assert!(restored.contains("ok"));
    }

    #[test]
    fn load_json_merges_and_overwrites() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &[]);
        let incoming = json!([
            {"key": "a", "value": 99},
            {"key": "b", "value": 2}
        ]);
        let loaded = s.load_json(&incoming);
        assert_eq!(loaded, 2);
        assert_eq!(s.len(), 2);
        assert_eq!(s.peek("a").unwrap().value, json!(99));
        assert_eq!(s.peek("b").unwrap().value, json!(2));
    }

    #[test]
    fn load_json_returns_zero_for_non_array() {
        let mut s = MemoryStore::new();
        assert_eq!(s.load_json(&json!({})), 0);
        assert!(s.is_empty());
    }

    #[test]
    fn prune_lru_keeps_most_recently_accessed() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &[]);
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.store("b", json!(2), &[]);
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.store("c", json!(3), &[]);
        // Touch `a` so it becomes most-recently accessed.
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.get("a");
        let removed = s.prune_lru(2);
        assert_eq!(removed, 1);
        assert_eq!(s.len(), 2);
        assert!(s.contains("a"));
        assert!(s.contains("c"));
        assert!(!s.contains("b"));
    }

    #[test]
    fn prune_lru_no_op_when_under_limit() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &[]);
        assert_eq!(s.prune_lru(5), 0);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn prune_lru_zero_clears() {
        let mut s = MemoryStore::new();
        s.store("a", json!(1), &[]);
        s.store("b", json!(2), &[]);
        assert_eq!(s.prune_lru(0), 2);
        assert!(s.is_empty());
    }
}
