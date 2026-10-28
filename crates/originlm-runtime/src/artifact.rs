//! Bounded, expiring, memory-only results. Provider paths are never accepted.
use crate::{engine::id, err};
use base64::{Engine, engine::general_purpose::STANDARD};
use originlm_contracts::*;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
const ENTRY_LIMIT: usize = 4 * 1024 * 1024;
const STORE_LIMIT: usize = 32 * 1024 * 1024;
struct Entry {
    owner: String,
    session: String,
    name: String,
    media: String,
    bytes: Vec<u8>,
    created: Instant,
}
pub(crate) struct ArtifactStore {
    entries: BTreeMap<String, Entry>,
    bytes: usize,
    ttl: Duration,
}
impl Default for ArtifactStore {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            bytes: 0,
            ttl: Duration::from_secs(900),
        }
    }
}
impl ArtifactStore {
    fn prune(&mut self) {
        let ttl = self.ttl;
        self.entries.retain(|_, e| e.created.elapsed() < ttl);
        self.bytes = self.entries.values().map(|e| e.bytes.len()).sum();
    }
    pub fn insert(
        &mut self,
        owner: &str,
        session: &str,
        name: &str,
        media: &str,
        bytes: Vec<u8>,
    ) -> Result<String> {
        if name.is_empty()
            || name.len() > 128
            || name.contains(['/', '\\'])
            || name.chars().any(char::is_control)
            || name == "."
            || name == ".."
            || media.len() > 128
            || !media.contains('/')
            || media
                .bytes()
                .any(|b| !b.is_ascii_alphanumeric() && !b"/-+.;= ".contains(&b))
        {
            return Err(err(ErrorCode::InvalidRequest));
        }
        self.prune();
        if bytes.len() > ENTRY_LIMIT
            || self.entries.len() >= 128
            || self.bytes + bytes.len() > STORE_LIMIT
        {
            return Err(err(ErrorCode::FrameTooLarge));
        }
        let key = id("artifact");
        self.bytes += bytes.len();
        self.entries.insert(
            key.clone(),
            Entry {
                owner: owner.into(),
                session: session.into(),
                name: name.into(),
                media: media.into(),
                bytes,
                created: Instant::now(),
            },
        );
        Ok(key)
    }
    pub fn read(
        &mut self,
        owner: &str,
        session: &str,
        key: &str,
        offset: u64,
        max: usize,
    ) -> Result<ArtifactChunk> {
        self.prune();
        let e = self
            .entries
            .get(key)
            .filter(|e| e.owner == owner && e.session == session)
            .ok_or_else(|| err(ErrorCode::NotFound))?;
        if max == 0 || max > 49152 || offset > e.bytes.len() as u64 {
            return Err(err(ErrorCode::InvalidRequest));
        }
        let begin = offset as usize;
        let end = (begin + max).min(e.bytes.len());
        Ok(ArtifactChunk {
            artifact_id: key.into(),
            session_id: session.into(),
            display_name: e.name.clone(),
            media_type: e.media.clone(),
            total_bytes: e.bytes.len().to_string(),
            offset: offset.to_string(),
            next_offset: end.to_string(),
            data_base64: STANDARD.encode(&e.bytes[begin..end]),
            eof: end == e.bytes.len(),
        })
    }
    pub fn remove(&mut self, key: &str) {
        if let Some(e) = self.entries.remove(key) {
            self.bytes -= e.bytes.len();
        }
    }
    pub fn clear_session(&mut self, session: &str) {
        self.entries.retain(|_, e| e.session != session);
        self.bytes = self.entries.values().map(|e| e.bytes.len()).sum();
    }
    pub fn clear_owner(&mut self, owner: &str) {
        self.entries.retain(|_, e| e.owner != owner);
        self.bytes = self.entries.values().map(|e| e.bytes.len()).sum();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_results_are_rejected() {
        assert!(
            ArtifactStore::default()
                .insert("a", "s", "safe", "text/plain", vec![0; ENTRY_LIMIT + 1])
                .is_err()
        );
    }
    #[test]
    fn expired_results_release_capacity_and_are_not_returned() {
        let mut s = ArtifactStore::default();
        let key = s.insert("a", "s", "safe", "text/plain", vec![1]).unwrap();
        s.ttl = Duration::ZERO;
        assert!(s.read("a", "s", &key, 0, 1).is_err());
        assert_eq!(s.bytes, 0);
    }
    #[test]
    fn result_ids_do_not_cross_session_boundaries() {
        let mut s = ArtifactStore::default();
        let key = s.insert("a", "s", "safe", "text/plain", vec![1]).unwrap();
        assert!(s.read("a", "other", &key, 0, 1).is_err());
    }
}
