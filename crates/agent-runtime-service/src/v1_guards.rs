use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use uuid::Uuid;

pub struct IdempotencyStore {
    entries: Mutex<HashMap<String, IdempotencyEntry>>,
}

#[derive(Clone)]
pub struct IdempotencyEntry {
    pub created_at: DateTime<Utc>,
    pub response_json: String,
    pub status_code: u16,
}

impl IdempotencyStore {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
    pub fn get(&self, key: &str) -> Option<IdempotencyEntry> {
        self.entries.lock().unwrap().get(key).cloned()
    }
    pub fn insert(&self, key: String, entry: IdempotencyEntry) {
        self.entries.lock().unwrap().insert(key, entry);
    }
}

pub struct RunGate {
    active: Mutex<HashSet<Uuid>>,
}

impl RunGate {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashSet::new()),
        }
    }
    pub fn try_acquire(&self, id: Uuid) -> bool {
        self.active.lock().unwrap().insert(id)
    }
    pub fn release(&self, id: Uuid) {
        self.active.lock().unwrap().remove(&id);
    }
}
