//! # Types
//!
//! Types used throughout the application
//!
//! Redis is originally a simple in-memory key-value data store.
//!
//! Our storage contains two data structures:
//! - one that maps keys to values,
//! - the other that maps keys to their expiration times, but only if the time is set for the key.
//!
//! In this way we save on storage space, as many keys might not have expiration time.
//!   - From [EXPIRE](https://redis.io/docs/latest/commands/expire/):
//!     "Normally, Redis keys are created without an associated time to live."

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
// use tokio::sync::RwLock;
use std::sync::RwLock;

/// Primary key
pub type StorageKey = String;
/// Stored value
pub type StorageValue = String;
/// Raw (inner) type of expiration time in milliseconds of an entry in the storage. Relevant only if the time is set.
pub type ExpirationTimeType = u128;
/// Expiration time of an entry in the storage. Wraps as an [`Option`] around [`ExpirationTimeType`].
pub type ExpirationTime = Option<ExpirationTimeType>;
/// The type of a single row (of a single stored entry): storage value
pub type StorageEntry = StorageValue;
/// A concrete in-memory storage implementation of the main key-value store - a hash map
pub type InMemoryStorageHashMap = HashMap<StorageKey, StorageEntry>;
/// A generic implementation of the auxiliary data structure that's used to store keys' expiration times
pub type InMemoryExpiryTime<DS> = DS;
/// A concrete implementation of the auxiliary data structure that's used to store keys' expiration times - a hash map
pub type InMemoryExpiryTimeHashMap = HashMap<StorageKey, ExpirationTime>;
/// A concrete implementation of the auxiliary data structure that's used to store keys' expiration times - a b-tree map
pub type InMemoryExpiryTimeBTreeMap = BTreeMap<StorageKey, ExpirationTime>;
/// Generic in-memory storage - could be a [`HashMap`] or a [`BTreeMap`] or anything else that resides in memory
pub type InMemoryStorage<KV, KE> = (KV, KE);
/// Generic storage type - could be [in-memory](crate::storage::inmemory) or file or DB or anything else
pub type StorageType<KV, KE> = InMemoryStorage<KV, KE>;
/// Wrapper around [`StorageType`] which makes it concurrent-safe
pub type ConcurrentStorageType<KV, KE> = Arc<RwLock<StorageType<KV, KE>>>;
