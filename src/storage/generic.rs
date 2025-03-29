//! Generic Storage For Our Redis Server - Data Abstraction Layer (DAL)
//!
//! - https://redis.io/nosql/what-is-nosql/
//! - https://redis.io/nosql/key-value-databases/
//!
//! Redis is originally a simple in-memory key-value data store.
//!
//! While Redis is originally implemented as a hash map, we leave the option to make its storage some other
//! in-memory data structure, such as a binary tree map, or even not-in-memory.
//!
//! The same is true of the auxiliary data structure that's used to store keys' expiration times.
//!
//! So, we use two data structures:
//! 1. One as the main Key-Value store,
//! 2. The other one as the Key-Expiry time store.
//!
//! In this way we may save a lot of storage space (memory) in general, as we only store expiry times for keys
//! for which they are `SET`, and not for all keys in the form of `None` or similar.
//!
//! - From [EXPIRE](https://redis.io/docs/latest/commands/expire/):
//!     "Normally, Redis keys are created without an associated time to live."

use crate::types::{ExpirationTime, StorageKey, StorageValue};

/// Trait: Generic storage - Data Abstraction Layer (DAL)
///
/// The generic parameter [`S`] is used to constrain the types that can be used as storage.
///
/// The trait itself makes the storage type generic (DAL), but the trait is then further type-bound.
///
/// [`KV`] is the main Key-Value store.
///
/// [`KE`] is the Key-Expiry time store.
pub trait Storage<S, KV, KE>
where
    S: Crud + Send + Sync + 'static,
    KV: SubStorage<S>,
    KE: SubStorage<S>,
{
    /// Create an instance of the storage
    fn new() -> Self;
}

/// This trait is used for Key-Value and Key-Expiry time stores.
pub trait SubStorage<S>
where
    S: Crud + Send + Sync + 'static,
{
    fn new() -> Self;
}

/// Trait CRUD: Create, Read, Update, Delete
pub trait Crud {
    /// Create an element
    fn create(&mut self, key: StorageKey, value: StorageValue, expiry: ExpirationTime);

    /// Read an element
    fn read(&self, key: StorageKey) -> Option<(StorageValue, ExpirationTime)>;

    /// Update an element
    ///
    /// Creates the entry if it didn't exist.
    fn update(&mut self, key: StorageKey, value: StorageValue, expiry: ExpirationTime) {
        self.create(key, value, expiry);
    }

    /// Delete an element
    fn delete(&mut self, key: StorageKey);
}
