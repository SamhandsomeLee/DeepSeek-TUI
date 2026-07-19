//! Small session-scoped TTL cache for fetched response bodies.

use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use lru::LruCache;
use parking_lot::Mutex;

const FETCH_CACHE_ENTRIES: usize = 256;
const FETCH_CACHE_TTL: Duration = Duration::from_secs(15 * 60);

static FETCH_CACHE: OnceLock<Mutex<LruCache<FetchCacheKey, FetchCacheEntry>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FetchCacheKey {
    namespace: String,
    url: String,
    accept: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedFetch {
    pub(crate) url: String,
    pub(crate) status: u16,
    pub(crate) headers: std::collections::BTreeMap<String, String>,
    pub(crate) content_type: String,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) truncated: bool,
    pub(crate) redirects: usize,
}

#[derive(Debug, Clone)]
struct FetchCacheEntry {
    fetched_at: Instant,
    payload: CachedFetch,
}

fn cache() -> &'static Mutex<LruCache<FetchCacheKey, FetchCacheEntry>> {
    FETCH_CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(FETCH_CACHE_ENTRIES).expect("non-zero cache capacity"),
        ))
    })
}

fn key(namespace: &str, url: &reqwest::Url, accept: &str) -> FetchCacheKey {
    let mut canonical = url.clone();
    canonical.set_fragment(None);
    FetchCacheKey {
        namespace: namespace.to_string(),
        url: canonical.to_string(),
        accept: accept.to_string(),
    }
}

pub(crate) fn get(
    namespace: &str,
    url: &reqwest::Url,
    accept: &str,
    max_bytes: usize,
) -> Option<CachedFetch> {
    let key = key(namespace, url, accept);
    let mut cache = cache().lock();
    let entry = cache.get(&key)?.clone();
    if entry.fetched_at.elapsed() > FETCH_CACHE_TTL {
        cache.pop(&key);
        return None;
    }

    // A truncated entry can answer an equal or smaller request. Asking for
    // more is an explicit refetch so the cached cap never becomes permanent.
    if entry.payload.truncated && max_bytes > entry.payload.bytes.len() {
        cache.pop(&key);
        return None;
    }

    let mut payload = entry.payload;
    if payload.bytes.len() > max_bytes {
        payload.bytes = Arc::new(payload.bytes[..max_bytes].to_vec());
        payload.truncated = true;
    }
    Some(payload)
}

pub(crate) fn insert(namespace: &str, url: &reqwest::Url, accept: &str, payload: CachedFetch) {
    cache().lock().put(
        key(namespace, url, accept),
        FetchCacheEntry {
            fetched_at: Instant::now(),
            payload,
        },
    );
}

#[cfg(test)]
pub(crate) fn reset() {
    cache().lock().clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(bytes: &[u8], truncated: bool) -> CachedFetch {
        CachedFetch {
            url: "https://example.com/doc".to_string(),
            status: 200,
            headers: Default::default(),
            content_type: "text/plain".to_string(),
            bytes: Arc::new(bytes.to_vec()),
            truncated,
            redirects: 0,
        }
    }

    #[test]
    fn truncated_entry_refetches_only_when_request_asks_for_more() {
        reset();
        let url = reqwest::Url::parse("https://example.com/doc#fragment").unwrap();
        insert("cache-unit", &url, "text/plain", payload(b"12345", true));

        let same = get("cache-unit", &url, "text/plain", 5).expect("same cap hit");
        assert!(same.truncated);
        let smaller = get("cache-unit", &url, "text/plain", 3).expect("smaller cap hit");
        assert_eq!(&*smaller.bytes, b"123");
        assert!(smaller.truncated);
        assert!(get("cache-unit", &url, "text/plain", 6).is_none());
    }

    #[test]
    fn cache_is_scoped_by_session_and_accept_header() {
        reset();
        let url = reqwest::Url::parse("https://example.com/doc").unwrap();
        insert("session-a", &url, "text/html", payload(b"body", false));

        assert!(get("session-a", &url, "text/html", 10).is_some());
        assert!(get("session-b", &url, "text/html", 10).is_none());
        assert!(get("session-a", &url, "application/json", 10).is_none());
    }
}
