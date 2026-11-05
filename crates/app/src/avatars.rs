//! Avatars from Gravatar (M5-11, ADR-026). Opt-in: with the setting off no
//! HTTP client exists and no request is made. Results and misses are cached
//! on disk for 7 days; at most 4 requests run at once, only for the authors
//! on screen.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

/// Cached results (and misses) are fresh for this long.
pub const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Requests running at the same time.
pub const MAX_PARALLEL: usize = 4;
/// Requested image size in pixels (sharp at 200 % zoom).
pub const SIZE: u32 = 64;

/// Gravatar hash: SHA-256 of the trimmed, lower case email address.
pub fn gravatar_hash(email: &str) -> String {
    let digest = Sha256::digest(email.trim().to_lowercase().as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// `d=404`: no generated placeholder; a missing avatar is a 404.
pub fn gravatar_url(hash: &str, size: u32) -> String {
    format!("https://gravatar.com/avatar/{hash}?s={size}&d=404")
}

/// Downloads an avatar: `Ok(None)` when Gravatar has none (404).
pub trait Fetcher: Send + Sync {
    fn fetch(&self, url: &str) -> Result<Option<Vec<u8>>, String>;
}

/// Fetcher over HTTPS (rustls; proxy from `HTTPS_PROXY`/`ALL_PROXY`/
/// `NO_PROXY`).
pub struct HttpFetcher {
    agent: ureq::Agent,
}

impl HttpFetcher {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .user_agent(git_core::identity::user_agent(env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self, url: &str) -> Result<Option<Vec<u8>>, String> {
        let mut response = self.agent.get(url).call().map_err(|e| e.to_string())?;
        match response.status().as_u16() {
            200 => response
                .body_mut()
                .with_config()
                .limit(1024 * 1024)
                .read_to_vec()
                .map(Some)
                .map_err(|e| e.to_string()),
            404 => Ok(None),
            status => Err(format!("Gravatar answered {status}")),
        }
    }
}

/// An avatar or the knowledge that there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cached {
    Image(Vec<u8>),
    Miss,
}

/// `<cache dir>/gitloom/avatars/<hash>.img` or `<hash>.miss`; the file's
/// modification time decides whether it is still fresh.
#[derive(Debug, Clone)]
pub struct DiskCache {
    dir: PathBuf,
}

impl DiskCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn default_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", git_core::identity::DIR_NAME)
            .map(|dirs| dirs.cache_dir().join("avatars"))
    }

    /// Fresh entry for `hash` at `now`, if any.
    pub fn get(&self, hash: &str, now: SystemTime) -> Option<Cached> {
        let fresh = |path: &PathBuf| {
            std::fs::metadata(path)
                .and_then(|m| m.modified())
                .is_ok_and(|modified| now.duration_since(modified).unwrap_or_default() < TTL)
        };
        let image = self.dir.join(format!("{hash}.img"));
        if fresh(&image) {
            return std::fs::read(&image).ok().map(Cached::Image);
        }
        fresh(&self.dir.join(format!("{hash}.miss"))).then_some(Cached::Miss)
    }

    pub fn put(&self, hash: &str, value: &Cached) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let (name, data, other): (String, &[u8], String) = match value {
            Cached::Image(bytes) => (format!("{hash}.img"), bytes, format!("{hash}.miss")),
            Cached::Miss => (format!("{hash}.miss"), b"", format!("{hash}.img")),
        };
        let _ = std::fs::remove_file(self.dir.join(other));
        let temp = self.dir.join(format!("{name}.tmp"));
        std::fs::write(&temp, data)?;
        std::fs::rename(&temp, self.dir.join(name))
    }
}

/// Loads one avatar: from the cache when fresh, else from the network
/// (the result, also a miss, is cached). Blocking.
pub fn load(
    hash: &str,
    cache: Option<&DiskCache>,
    fetcher: &dyn Fetcher,
    now: SystemTime,
) -> Result<Cached, String> {
    if let Some(cached) = cache.and_then(|c| c.get(hash, now)) {
        return Ok(cached);
    }
    let result = match fetcher.fetch(&gravatar_url(hash, SIZE))? {
        Some(bytes) => Cached::Image(bytes),
        None => Cached::Miss,
    };
    if let Some(cache) = cache {
        // A cache that cannot be written only costs a request next time.
        let _ = cache.put(hash, &result);
    }
    Ok(result)
}

#[derive(Debug, Clone)]
pub enum State {
    Queued,
    Loading,
    Ready(iced::widget::image::Handle),
    /// No avatar (or loading failed): initials stay.
    Missing,
}

/// Avatars of the authors seen so far and the request queue.
#[derive(Debug, Default)]
pub struct Avatars {
    /// Email (as in the commit) → hash.
    hashes: HashMap<String, String>,
    states: HashMap<String, State>,
    queue: VecDeque<String>,
    in_flight: usize,
}

impl Avatars {
    /// Asks for the avatars of `emails` (the authors on screen) and returns
    /// the hashes to load now; the rest waits until a slot is free.
    pub fn request<'a>(&mut self, emails: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let mut seen = HashSet::new();
        for email in emails {
            if email.trim().is_empty() || !seen.insert(email) {
                continue;
            }
            // Called on every update: no allocation for known authors.
            let hash = match self.hashes.get(email) {
                Some(hash) => hash.clone(),
                None => {
                    let hash = gravatar_hash(email);
                    self.hashes.insert(email.to_owned(), hash.clone());
                    hash
                }
            };
            if !self.states.contains_key(&hash) {
                self.states.insert(hash.clone(), State::Queued);
                self.queue.push_back(hash);
            }
        }
        self.start()
    }

    /// One request finished; returns the next hashes to load.
    pub fn finished(&mut self, hash: &str, result: Result<Cached, String>) -> Vec<String> {
        self.in_flight = self.in_flight.saturating_sub(1);
        let state = match result {
            Ok(Cached::Image(bytes)) => {
                State::Ready(iced::widget::image::Handle::from_bytes(bytes))
            }
            Ok(Cached::Miss) | Err(_) => State::Missing,
        };
        self.states.insert(hash.to_owned(), state);
        self.start()
    }

    fn start(&mut self) -> Vec<String> {
        let mut started = Vec::new();
        while self.in_flight < MAX_PARALLEL {
            let Some(hash) = self.queue.pop_front() else {
                break;
            };
            self.in_flight += 1;
            self.states.insert(hash.clone(), State::Loading);
            started.push(hash);
        }
        started
    }

    /// Loaded avatar of the author with `email`.
    pub fn get(&self, email: &str) -> Option<&iced::widget::image::Handle> {
        let hash = self.hashes.get(email)?;
        match self.states.get(hash)? {
            State::Ready(handle) => Some(handle),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn in_flight(&self) -> usize {
        self.in_flight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn hash_of_trimmed_lower_case_email() {
        // SHA-256 of "test@example.com".
        let expected = "973dfe463ec85785f5f95af5ba3906eedb2d931c24e69824a89ea65dba4e813b";
        assert_eq!(gravatar_hash("test@example.com"), expected);
        assert_eq!(gravatar_hash("  Test@Example.COM \n"), expected);
        assert_eq!(
            gravatar_url(expected, 64),
            format!("https://gravatar.com/avatar/{expected}?s=64&d=404")
        );
    }

    fn temp_cache(name: &str) -> DiskCache {
        let dir =
            std::env::temp_dir().join(format!("gitloom-avatars-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        DiskCache::new(dir)
    }

    struct Fake {
        calls: AtomicUsize,
        answer: Option<Vec<u8>>,
    }

    impl Fetcher for Fake {
        fn fetch(&self, _url: &str) -> Result<Option<Vec<u8>>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.answer.clone())
        }
    }

    #[test]
    fn cache_keeps_images_and_misses_for_seven_days() {
        let cache = temp_cache("ttl");
        let now = SystemTime::now();
        let image = Fake {
            calls: AtomicUsize::new(0),
            answer: Some(vec![1, 2, 3]),
        };
        assert_eq!(
            load("h1", Some(&cache), &image, now),
            Ok(Cached::Image(vec![1, 2, 3]))
        );
        assert_eq!(
            load("h1", Some(&cache), &image, now),
            Ok(Cached::Image(vec![1, 2, 3]))
        );
        assert_eq!(
            image.calls.load(Ordering::SeqCst),
            1,
            "second load from the cache"
        );

        let missing = Fake {
            calls: AtomicUsize::new(0),
            answer: None,
        };
        assert_eq!(load("h2", Some(&cache), &missing, now), Ok(Cached::Miss));
        assert_eq!(load("h2", Some(&cache), &missing, now), Ok(Cached::Miss));
        assert_eq!(
            missing.calls.load(Ordering::SeqCst),
            1,
            "misses are cached too"
        );

        // After the TTL both are stale.
        let later = now + TTL + Duration::from_secs(60);
        assert_eq!(cache.get("h1", later), None);
        assert_eq!(cache.get("h2", later), None);
        assert_eq!(load("h2", Some(&cache), &missing, later), Ok(Cached::Miss));
        assert_eq!(missing.calls.load(Ordering::SeqCst), 2);

        // An image replaces an old miss.
        cache.put("h2", &Cached::Image(vec![9])).unwrap();
        assert_eq!(cache.get("h2", now), Some(Cached::Image(vec![9])));
        let _ = std::fs::remove_dir_all(&cache.dir);
    }

    #[test]
    fn at_most_four_requests_run_at_once() {
        let mut avatars = Avatars::default();
        let emails: Vec<String> = (0..6).map(|i| format!("dev{i}@example.com")).collect();
        let started = avatars.request(emails.iter().map(String::as_str));
        assert_eq!(started.len(), MAX_PARALLEL);
        assert_eq!(avatars.in_flight(), 4);
        // Asking again starts nothing new.
        assert!(avatars
            .request(emails.iter().map(String::as_str))
            .is_empty());
        let next = avatars.finished(&started[0], Ok(Cached::Miss));
        assert_eq!(next.len(), 1);
        assert_eq!(avatars.in_flight(), 4);
        assert!(
            avatars.get("dev0@example.com").is_none(),
            "missing: initials stay"
        );
        let next = avatars.finished(&started[1], Ok(Cached::Image(vec![1])));
        assert_eq!(next.len(), 1);
        assert!(avatars.get("dev1@example.com").is_some());
        // Empty emails and duplicates are skipped.
        assert!(avatars.request(["", " ", "dev1@example.com"]).is_empty());
    }
}
