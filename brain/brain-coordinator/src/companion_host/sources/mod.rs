use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

/// A polled observation source.
///
/// The companion host polls all registered sources on each cycle.
/// Return `None` if nothing meaningful to report.
#[async_trait::async_trait]
pub trait ObservationSource: Debug + Send + Sync {
    fn name(&self) -> &'static str;
    async fn poll(&self) -> Option<String>;
}

/// Change detector: tracks content hashes per source and filters
/// out duplicate observations.
#[derive(Debug)]
pub struct ChangeDetector {
    digests: Mutex<std::collections::HashMap<&'static str, u64>>,
    skipped: Mutex<u64>,
}

impl ChangeDetector {
    pub fn new() -> Self {
        Self {
            digests: Mutex::new(std::collections::HashMap::new()),
            skipped: Mutex::new(0),
        }
    }

    pub fn is_changed(&self, source: &'static str, content: &str) -> bool {
        let digest = content_digest(content);
        let mut digests = self.digests.lock().unwrap();
        let last = digests.get(source).copied();
        if last == Some(digest) {
            *self.skipped.lock().unwrap() += 1;
            return false;
        }
        digests.insert(source, digest);
        true
    }

    pub fn skipped_count(&self) -> u64 {
        *self.skipped.lock().unwrap()
    }

    pub fn reset_skipped(&self) {
        *self.skipped.lock().unwrap() = 0;
    }
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

fn content_digest(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

mod desktop;
pub use desktop::FnDesktopProvider;
pub use desktop::{DesktopProvider, DesktopSource};

mod time_source;
pub use time_source::TimeSource;

mod system_source;
pub use system_source::SystemSource;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct CountingSource {
        count: Mutex<u64>,
    }

    impl CountingSource {
        fn new() -> Self {
            Self {
                count: Mutex::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl ObservationSource for CountingSource {
        fn name(&self) -> &'static str {
            "counter"
        }

        async fn poll(&self) -> Option<String> {
            let mut c = self.count.lock().unwrap();
            *c += 1;
            Some(format!("count: {}", *c))
        }
    }

    #[test]
    fn test_change_detector_rejects_duplicates() {
        let detector = ChangeDetector::new();
        assert!(detector.is_changed("test", "hello"));
        assert!(!detector.is_changed("test", "hello"));
        assert!(detector.is_changed("test", "world"));
        assert_eq!(detector.skipped_count(), 1);
    }

    #[test]
    fn test_change_detector_per_source_separate() {
        let detector = ChangeDetector::new();
        assert!(detector.is_changed("a", "same"));
        assert!(detector.is_changed("b", "same"));
        assert!(!detector.is_changed("a", "same"));
        assert!(!detector.is_changed("b", "same"));
    }

    #[tokio::test]
    async fn test_counting_source_increases() {
        let source = CountingSource::new();
        let r1 = source.poll().await;
        let r2 = source.poll().await;
        assert!(r1.is_some());
        assert!(r2.is_some());
        assert_ne!(r1, r2);
    }
}
