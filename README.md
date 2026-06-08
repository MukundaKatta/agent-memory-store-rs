# agent-memory-store

[![CI](https://github.com/MukundaKatta/agent-memory-store-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/MukundaKatta/agent-memory-store-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](#license)

A small, dependency-light key-value **memory store for AI agents**. Persist
facts, preferences, and summaries across conversation turns, group them with
tags, and snapshot the whole store to JSON so it survives between sessions.

The store is intentionally simple: it is an in-memory `HashMap` wrapper with
per-entry bookkeeping (creation time, last-access time, access count), tag-based
retrieval, JSON serialization round-trip, and LRU pruning to keep long-running
agents bounded.

## Features

- **Key-value storage** of arbitrary `serde_json::Value` payloads.
- **Tagging** for grouped retrieval (`by_tag`, `tags`).
- **Access tracking** — `get` records last-access time and a hit counter, while
  `peek` reads without mutating metadata.
- **JSON round-trip** — `to_json` / `from_json` / `load_json` for persistence and
  merging snapshots.
- **LRU pruning** — `prune_lru` caps the store to the N most recently accessed
  entries.
- **Tiny dependency footprint** — only `serde_json`.

## Install

Add it to your `Cargo.toml`:

```toml
[dependencies]
agent-memory-store = { git = "https://github.com/MukundaKatta/agent-memory-store-rs" }
serde_json = "1"
```

## Usage

```rust
use agent_memory_store::MemoryStore;
use serde_json::json;

fn main() {
    let mut store = MemoryStore::new();

    // Record facts learned during a conversation.
    store.store("user_name", json!("Alice"), &["user", "profile"]);
    store.store("user_city", json!("Paris"), &["user", "location"]);

    // Retrieve a single fact (this updates access metadata).
    if let Some(entry) = store.get("user_name") {
        println!("name = {}", entry.value); // "Alice"
        println!("read {} time(s)", entry.access_count);
    }

    // Grouped retrieval by tag (sorted by key, deterministic).
    for entry in store.by_tag("user") {
        println!("{} = {}", entry.key, entry.value);
    }

    // List every distinct tag.
    assert_eq!(store.tags(), vec!["location", "profile", "user"]);

    // Snapshot to JSON for persistence, then restore it later.
    let snapshot = store.to_json();
    let restored = MemoryStore::from_json(&snapshot).unwrap();
    assert_eq!(restored.len(), 2);

    // Keep memory bounded: retain only the 1 most-recently-accessed entry.
    let mut bounded = store.clone();
    let removed = bounded.prune_lru(1);
    println!("pruned {removed} stale entries");
}
```

## API

### `MemoryStore`

| Method | Description |
| --- | --- |
| `new() -> MemoryStore` | Create an empty store. |
| `store(key, value, tags)` | Insert or overwrite an entry. Preserves `created_at` and `access_count` on overwrite. |
| `get(key) -> Option<&MemoryEntry>` | Fetch an entry, updating `accessed_at` and incrementing `access_count`. |
| `peek(key) -> Option<&MemoryEntry>` | Fetch an entry without touching access metadata. |
| `delete(key) -> bool` | Remove an entry; `true` if it existed. |
| `contains(key) -> bool` | Whether a key exists. |
| `all() -> Vec<&MemoryEntry>` | All entries, sorted by key. |
| `by_tag(tag) -> Vec<&MemoryEntry>` | Entries carrying `tag`, sorted by key. |
| `tags() -> Vec<String>` | All distinct tags, sorted alphabetically. |
| `keys() -> Vec<&str>` | All keys, sorted. |
| `len() -> usize` / `is_empty() -> bool` | Size helpers. |
| `clear()` | Remove all entries. |
| `to_json() -> Value` | Serialize the store to a JSON array. |
| `from_json(&Value) -> Option<MemoryStore>` | Rebuild a store from a JSON array; unparseable elements are skipped, `None` if not an array. |
| `load_json(&Value) -> usize` | Merge a JSON array into this store (overwrites on key conflict); returns the number loaded. |
| `prune_lru(max_entries) -> usize` | Evict least-recently-accessed entries until `max_entries` remain; returns the number removed. |

### `MemoryEntry`

Fields: `key`, `value`, `tags`, `created_at`, `accessed_at`, `access_count`.

| Method | Description |
| --- | --- |
| `to_json() -> Value` | Serialize the entry to a JSON object. |
| `from_json(&Value) -> Option<MemoryEntry>` | Reconstruct an entry; requires a string `key`, tolerates missing optional fields. |

## Development

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Continuous integration runs all of the above on every push and pull request via
[GitHub Actions](.github/workflows/ci.yml).

## License

Licensed under the [MIT License](https://opensource.org/licenses/MIT).
