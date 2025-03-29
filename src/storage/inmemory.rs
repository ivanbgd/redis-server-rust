//! In-memory (not-persistent) representation of a CRUD storage

use crate::storage::generic::{Crud, SubStorage};
use crate::storage::Storage;
use crate::types::{
    ExpirationTime, InMemoryExpiryTimeHashMap, InMemoryStorage, InMemoryStorageHashMap, StorageKey,
    StorageValue,
};

impl<S, KV, KE> Storage<S, KV, KE> for InMemoryStorage<KV, KE>
where
    S: Crud + Sync + Send + 'static,
    KV: SubStorage<S>,
    KE: SubStorage<S>,
{
    fn new() -> Self {
        (KV::new(), KE::new())
    }
}

impl<S> SubStorage<S> for InMemoryStorageHashMap
where
    S: Crud + Sync + Send + 'static,
{
    fn new() -> Self {
        Self::new()
    }
}

impl<S> SubStorage<S> for InMemoryExpiryTimeHashMap
where
    S: Crud + Sync + Send + 'static,
{
    fn new() -> Self {
        Self::new()
    }
}

impl<KV: Crud, KE: Crud> Crud for InMemoryStorage<KV, KE> {
    fn create(&mut self, key: StorageKey, value: StorageValue, expiry: ExpirationTime) {
        self.0.create(key.clone(), value.clone(), expiry);
        if expiry.is_some() {
            self.1.create(key, value, expiry)
        }
    }

    fn read(&self, key: StorageKey) -> Option<(StorageValue, ExpirationTime)> {
        match self.0.read(key.clone()) {
            None => None,
            Some((value, _dummy_expiry)) => {
                let expiry = match self.1.read(key.clone()) {
                    Some((_dummy_value, expiry)) => expiry,
                    None => None,
                };
                Some((value, expiry))
            }
        }
    }

    fn delete(&mut self, key: StorageKey) {
        self.0.delete(key.clone());
        self.1.delete(key);
    }
}

impl Crud for InMemoryStorageHashMap {
    fn create(&mut self, key: StorageKey, value: StorageValue, _expiry: ExpirationTime) {
        self.insert(key, value);
    }

    fn read(&self, key: StorageKey) -> Option<(StorageValue, ExpirationTime)> {
        self.get(&key).map(|value| (value.clone(), None))
    }

    fn delete(&mut self, key: StorageKey) {
        self.remove(&key);
    }
}

impl Crud for InMemoryExpiryTimeHashMap {
    fn create(&mut self, key: StorageKey, _value: StorageValue, expiry: ExpirationTime) {
        self.insert(key, expiry);
    }

    fn read(&self, key: StorageKey) -> Option<(StorageValue, ExpirationTime)> {
        self.get(&key).map(|value| ("".to_string(), *value))
    }

    fn delete(&mut self, key: StorageKey) {
        self.remove(&key);
    }
}
