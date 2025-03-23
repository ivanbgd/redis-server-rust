//! In-memory (not-persistent) representation of a CRUD storage

use crate::constants::{StorageKey, StorageValue};
use crate::storage::generic::Crud;
use crate::storage::Storage;
use std::collections::HashMap;

impl<S> Storage<S> for HashMap<StorageKey, StorageValue>
where
    S: Crud + Sync + Send,
{
    fn new() -> Self {
        Self::new()
    }
}

impl Crud for HashMap<StorageKey, StorageValue> {
    fn create(&mut self, key: StorageKey, value: StorageValue) -> Option<StorageKey> {
        self.insert(key, value)
    }

    fn read(&self, key: StorageKey) -> Option<StorageValue> {
        self.get(&key).cloned()
    }

    fn delete(&mut self, key: StorageKey) {
        self.remove(&key);
    }
}
