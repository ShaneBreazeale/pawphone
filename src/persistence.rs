//! SwiftData's stand-in: a small JSON document store. Pure Rust (no C, no
//! bundled SQLite), so it builds anywhere the rest of the app does. Lives on
//! the main thread; seeds the cast of cats on first launch and persists every
//! message + delivery status by rewriting one small file on each mutation.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::models::{Cat, CatPersona, CatStatus, DeliveryStatus, Message};

/// Epoch milliseconds, for message ordering.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Default, Serialize, Deserialize)]
struct Db {
    cats: Vec<Cat>,
    messages: Vec<Message>,
    next_message_id: i64,
}

pub struct Store {
    /// `None` for the in-memory fallback (changes are not written to disk).
    path: Option<PathBuf>,
    db: Db,
}

impl Store {
    /// Open (or create) the store in the platform data dir, then seed.
    pub fn open() -> std::io::Result<Store> {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Db::default(),
        };
        let mut store = Store { path: Some(path), db };
        store.seed_if_empty();
        store.save();
        Ok(store)
    }

    /// In-memory store, used if disk access fails. Never persists.
    pub fn in_memory() -> Store {
        let mut store = Store { path: None, db: Db::default() };
        store.seed_if_empty();
        store
    }

    fn seed_if_empty(&mut self) {
        if !self.db.cats.is_empty() {
            return;
        }
        let seed: &[(&str, f32, CatStatus, CatPersona)] = &[
            ("Sir Whiskers", 0.82, CatStatus::Online, CatPersona::Aloof),
            ("Mittens", 0.61, CatStatus::Napping, CatPersona::Needy),
            ("The Orange One", 0.95, CatStatus::Online, CatPersona::Chaotic),
            ("Biscuit", 0.40, CatStatus::Napping, CatPersona::FoodObsessed),
        ];
        for (idx, (name, signal, status, persona)) in seed.iter().enumerate() {
            self.db.cats.push(Cat {
                id: idx as i64 + 1,
                name: name.to_string(),
                signal: *signal,
                status: *status,
                persona: *persona,
            });
        }
    }

    fn save(&self) {
        if let Some(path) = &self.path
            && let Ok(text) = serde_json::to_string_pretty(&self.db)
        {
            let _ = std::fs::write(path, text);
        }
    }

    pub fn load_cats(&self) -> Vec<Cat> {
        self.db.cats.clone()
    }

    pub fn load_messages(&self, cat_id: i64) -> Vec<Message> {
        let mut msgs: Vec<Message> =
            self.db.messages.iter().filter(|m| m.cat_id == cat_id).cloned().collect();
        msgs.sort_by_key(|m| (m.created_at, m.id));
        msgs
    }

    /// Insert a message; assigns and returns its id.
    pub fn insert_message(&mut self, m: &Message) -> i64 {
        self.db.next_message_id += 1;
        let id = self.db.next_message_id;
        let mut stored = m.clone();
        stored.id = id;
        self.db.messages.push(stored);
        self.save();
        id
    }

    pub fn update_message_status(&mut self, id: i64, status: DeliveryStatus) {
        if let Some(m) = self.db.messages.iter_mut().find(|m| m.id == id) {
            m.status = status;
        }
        self.save();
    }
}

fn db_path() -> PathBuf {
    if let Some(dirs) = directories::ProjectDirs::from("org", "PawLink", "PawPhone") {
        dirs.data_dir().join("pawphone.json")
    } else {
        PathBuf::from("pawphone.json")
    }
}
