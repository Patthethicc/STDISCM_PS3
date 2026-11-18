//! Thread-safe SHA-256 registry for duplicate detection.
//!
//! A hash is *reserved* atomically when an upload header arrives, so two
//! identical simultaneous uploads cannot both pass. If the upload is later
//! dropped or fails, call [`HashRegistry::release`] so a retry is not flagged
//! as a duplicate. Once the video is successfully saved, call
//! [`HashRegistry::commit`] to keep the hash permanently.

use std::collections::HashSet;
use std::sync::Mutex;

#[derive(Default)]
pub struct HashRegistry {
    inner: Mutex<State>,
}

#[derive(Default)]
struct State {
    reserved: HashSet<String>,
    committed: HashSet<String>,
}

impl HashRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically reserves `hash`. Returns `false` if it is already reserved
    /// (in flight) or committed (saved) — i.e. the upload is a duplicate.
    pub fn try_reserve(&self, hash: &str) -> bool {
        let mut s = self.inner.lock().unwrap();
        if s.committed.contains(hash) || s.reserved.contains(hash) {
            return false;
        }
        s.reserved.insert(hash.to_owned());
        true
    }

    /// Gives up a reservation (upload dropped or failed).
    pub fn release(&self, hash: &str) {
        self.inner.lock().unwrap().reserved.remove(hash);
    }

    /// Turns a reservation into a permanent entry (video saved).
    pub fn commit(&self, hash: &str) {
        let mut s = self.inner.lock().unwrap();
        s.reserved.remove(hash);
        s.committed.insert(hash.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn second_reserve_is_duplicate() {
        let r = HashRegistry::new();
        assert!(r.try_reserve("a"));
        assert!(!r.try_reserve("a"));
    }

    #[test]
    fn release_allows_retry() {
        let r = HashRegistry::new();
        assert!(r.try_reserve("a"));
        r.release("a");
        assert!(r.try_reserve("a"));
    }

    #[test]
    fn committed_stays_duplicate() {
        let r = HashRegistry::new();
        assert!(r.try_reserve("a"));
        r.commit("a");
        assert!(!r.try_reserve("a"));
    }

    #[test]
    fn concurrent_reserve_has_one_winner() {
        let r = Arc::new(HashRegistry::new());
        let wins: usize = (0..16)
            .map(|_| {
                let r = r.clone();
                std::thread::spawn(move || r.try_reserve("same"))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap() as usize)
            .sum();
        assert_eq!(wins, 1);
    }
}
