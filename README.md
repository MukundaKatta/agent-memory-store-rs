# agent-memory-store

A simple, dependency-light key-value **memory store for AI agents**, written in Rust.
It lets an agent persist facts, preferences, and summaries across conversation turns,
group them with tags for selective retrieval, and track access metadata for each entry.

## Features

- **Key-value storage** of arbitrary JSON values (`serde_json::Value`).
- **Tagging** — attach tags to entries and fetch them back by tag (`by_tag`).
- **Access tracking** — every `get` updates `accessed_at` and increments `access_count`,
  while `peek` reads without mutating metadata.
- **Stable ordering** — `all()` and `keys()` return entries sorted by key.
- **JSON serialization** — dump a single entry or the whole store to JSON.
- **Overwrite-safe** — re-storing a key preserves its original `created_at`.

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
agent-memory-store = "0.1"
serde_json = "1"
```

## Usage

```rust
use agent_memory_store::MemoryStore;
use serde_json::json;

let mut store = MemoryStore::new();

// Store a fact with tags.
store.store("user_name", json!("Alice"), &["user", "profile"]);
store.store("favorite_color", json!("blue"), &["user", "preference"]);

// Retrieve it (this updates access metadata).
let entry = store.get("user_name").unwrap();
assert_eq!(entry.value, json!("Alice"));

// Fetch everything sharing a tag.
let user_facts = store.by_tag("user");
assert_eq!(user_facts.len(), 2);

// Peek without affecting access counts.
let peeked = store.peek("favorite_color").unwrap();
assert_eq!(peeked.value, json!("blue"));

// Export the whole store as a JSON array.
let snapshot = store.to_json();
assert!(snapshot.is_array());
```

## API overview

| Method | Description |
| ------ | ----------- |
| `new()` | Create an empty store. |
| `store(key, value, tags)` | Insert or overwrite an entry; preserves `created_at` on overwrite. |
| `get(key)` | Fetch an entry and bump its access metadata. |
| `peek(key)` | Fetch an entry without mutating metadata. |
| `delete(key)` | Remove an entry; returns whether it existed. |
| `all()` | All entries, sorted by key. |
| `by_tag(tag)` | All entries carrying a given tag. |
| `keys()` | All keys, sorted. |
| `contains(key)` | Whether a key exists. |
| `len()` / `is_empty()` | Size helpers. |
| `clear()` | Remove all entries. |
| `to_json()` | Serialize the whole store to a JSON array. |

Each `MemoryEntry` exposes `key`, `value`, `tags`, `created_at`, `accessed_at`,
and `access_count`, plus a `to_json()` method.

## Tech stack

- **Language:** Rust (edition 2021)
- **Dependencies:** [`serde_json`](https://crates.io/crates/serde_json)
- Storage is in-memory via `std::collections::HashMap`.

## Development

```bash
cargo build      # build the library
cargo test       # run the unit test suite
cargo clippy     # lint
```

## License

Licensed under the [MIT License](https://opensource.org/licenses/MIT).
