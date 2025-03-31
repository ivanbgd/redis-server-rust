//! Eviction Facility
//!
//! Implementation of a background thread for eviction of expired keys.

use crate::constants::HZ_MS;
use crate::errors::CmdError;
use crate::storage::generic::Crud;
use crate::types::{ConcurrentStorageType, ExpirationTime, StorageKey};
use anyhow::Result;
use std::fmt::Debug;
use std::ops::DerefMut;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Removes expired keys from the storage
///
/// Meant to be run in a background thread as it loops infinitely.
///
/// It sleeps for [`Hz`](HZ_MS) milliseconds and then removes expired keys from the storage in a loop.
pub fn eviction_loop<
    KV: Crud + Debug,
    KE: Clone + Crud + Debug + IntoIterator<Item = (StorageKey, ExpirationTime)>,
>(
    storage: ConcurrentStorageType<KV, KE>,
) -> Result<(), CmdError> {
    eprintln!("Eviction!"); // todo rem
    let mut i = 0; // todo rem
    loop {
        let time_now_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(since) => since,
            Err(err) => return Err(CmdError::TimeError(err)),
        }
        .as_millis();
        eprintln!("{i}"); // todo rem
                          // tokio::time::sleep(Duration::from_millis(HZ as u64)).await;
                          // tokio::time::timeout(Duration::from_millis(HZ as u64)).await;
        let mut s = storage.write().expect("RwLockWriteGuard");
        // println!("{:?}", s.1);
        let (kv, ke) = s.deref_mut();
        eprintln!("KV: {kv:?}"); // todo keep
        eprintln!("KE: {ke:?}"); // todo keep
        for (key, expiry) in ke.clone().into_iter() {
            if time_now_ms > expiry.expect("Expected Some(expiry)") {
                kv.delete(&key);
                ke.delete(&key);
            }
        }
        std::thread::sleep(Duration::from_millis(HZ_MS as u64));
        i += 1; // todo rem
                // std::thread::yield_now();
    }
}
