//! Generic Storage For Our Redis Server - Data Abstraction Layer (DAL)

use crate::constants::{StorageKey, StorageValue};

/// Trait: Generic storage - Data Abstraction Layer (DAL)
///
/// The generic parameter [`S`] is used to constrain the types that can be used as storage.
///
/// The trait itself makes the storage type generic (DAL), but the trait is then further type-bound.
pub trait Storage<S>
where
    S: Crud + Sync + Send,
{
    /// Create an instance of the storage
    fn new() -> Self;
}

/// Trait CRUD: Create, Read, Update, Delete
pub trait Crud {
    /// Create an element
    fn create(&mut self, key: StorageKey, value: StorageValue) -> Option<StorageKey>;

    /// Read an element
    fn read(&self, key: StorageKey) -> Option<StorageValue>;

    /// Update an element
    fn update(&mut self, key: StorageKey, value: StorageValue) {
        self.create(key, value);
    }

    /// Delete an element
    fn delete(&mut self, key: StorageKey);
}
