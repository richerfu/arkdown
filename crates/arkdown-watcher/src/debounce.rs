//! Event debouncing for the filesystem watcher.
//!
//! Batches rapid-fire filesystem events into a single `DebouncedBatch` after
//! a configurable delay, preventing redundant rebuilds when editors write
//! multiple files in quick succession.
//!
//! Upstream: `arkts-pack.js` `buildDelay` / watch delay logic.

use std::time::{Duration, Instant};

use crate::watcher::WatchEvent;

// ---------------------------------------------------------------------------
// DebounceConfig
// ---------------------------------------------------------------------------

/// Configuration for the `Debouncer`.
#[derive(Debug, Clone)]
pub struct DebounceConfig {
    /// How long to wait after the last event before flushing the batch.
    ///
    /// Default: 150 ms (hvigor `buildDelay` is 100 ms; we add 50 ms headroom
    /// to account for editor multi-file saves).
    pub delay: Duration,
    /// Maximum number of events to accumulate before force-flushing.
    ///
    /// Prevents unbounded memory growth when a large refactor touches hundreds
    /// of files simultaneously.  Default: 50.
    pub max_batch_size: usize,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        DebounceConfig {
            delay: Duration::from_millis(150),
            max_batch_size: 50,
        }
    }
}

impl DebounceConfig {
    /// Create a config with the given delay and default max batch size.
    pub fn with_delay(delay: Duration) -> Self {
        DebounceConfig {
            delay,
            ..Default::default()
        }
    }

    /// Create a config with the given max batch size and default delay.
    pub fn with_max_batch_size(max_batch_size: usize) -> Self {
        DebounceConfig {
            max_batch_size,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// DebouncedBatch
// ---------------------------------------------------------------------------

/// A batch of `WatchEvent`s collected within a debounce window.
#[derive(Debug, Clone)]
pub struct DebouncedBatch {
    /// The events in this batch.
    pub events: Vec<WatchEvent>,
    /// Monotonic timestamp of when the batch was flushed.
    pub collected_at: Instant,
}

impl DebouncedBatch {
    /// Create a new batch.
    pub fn new(events: Vec<WatchEvent>) -> Self {
        DebouncedBatch {
            events,
            collected_at: Instant::now(),
        }
    }

    /// Return `true` if the batch contains no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Return the number of events in the batch.
    pub fn len(&self) -> usize {
        self.events.len()
    }
}

// ---------------------------------------------------------------------------
// Debouncer
// ---------------------------------------------------------------------------

/// Collects `WatchEvent`s and flushes them as a `DebouncedBatch` once the
/// debounce delay has elapsed or the max batch size is reached.
///
/// Typical usage:
///
/// ```
/// use std::time::Duration;
/// use arkdown_watcher::debounce::{Debouncer, DebounceConfig};
/// use arkdown_watcher::watcher::{WatchEvent, WatchEventKind};
/// use std::path::PathBuf;
///
/// let config = DebounceConfig::default();
/// let mut debouncer = Debouncer::new(config);
///
/// debouncer.push(WatchEvent::new(PathBuf::from("/a/b.ets"), WatchEventKind::Modified));
///
/// // No batch yet — delay has not elapsed.
/// assert!(debouncer.flush_ready().is_none());
///
/// // Force-flush returns whatever is pending.
/// let batch = debouncer.force_flush();
/// assert_eq!(batch.len(), 1);
/// ```
pub struct Debouncer {
    config: DebounceConfig,
    /// Pending events not yet flushed.
    pending: Vec<WatchEvent>,
    /// Monotonic time of the last `push` call.  `None` when the buffer is empty.
    last_push: Option<Instant>,
}

impl Debouncer {
    /// Create a new debouncer with the given configuration.
    pub fn new(config: DebounceConfig) -> Self {
        Debouncer {
            config,
            pending: Vec::new(),
            last_push: None,
        }
    }

    /// Push a new event into the pending buffer.
    ///
    /// Resets the debounce timer.  If the buffer has reached `max_batch_size`,
    /// the event is still accepted — call `flush_ready` or `force_flush` to
    /// drain it.
    pub fn push(&mut self, event: WatchEvent) {
        self.pending.push(event);
        self.last_push = Some(Instant::now());
    }

    /// Return a `DebouncedBatch` if the debounce delay has elapsed since the
    /// last `push`, or if the pending buffer has reached `max_batch_size`.
    ///
    /// Returns `None` when the buffer is empty or the delay has not elapsed.
    pub fn flush_ready(&mut self) -> Option<DebouncedBatch> {
        if self.pending.is_empty() {
            return None;
        }

        let should_flush = match self.last_push {
            None => false,
            Some(last) => {
                let elapsed = last.elapsed();
                elapsed >= self.config.delay || self.pending.len() >= self.config.max_batch_size
            }
        };

        if should_flush {
            Some(self.take_batch())
        } else {
            None
        }
    }

    /// Drain all pending events into a `DebouncedBatch` immediately, regardless
    /// of whether the debounce delay has elapsed.
    ///
    /// Returns an empty batch when there are no pending events.
    pub fn force_flush(&mut self) -> DebouncedBatch {
        self.take_batch()
    }

    /// Return `true` if there are pending events.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Return the number of pending events.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    // -----------------------------------------------------------------------
    // Private
    // -----------------------------------------------------------------------

    fn take_batch(&mut self) -> DebouncedBatch {
        let events = std::mem::take(&mut self.pending);
        self.last_push = None;
        DebouncedBatch::new(events)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::WatchEventKind;
    use std::path::PathBuf;

    fn make_event(path: &str) -> WatchEvent {
        WatchEvent::new(PathBuf::from(path), WatchEventKind::Modified)
    }

    #[test]
    fn test_debounce_config_default_values() {
        let cfg = DebounceConfig::default();
        assert_eq!(cfg.delay, Duration::from_millis(150));
        assert_eq!(cfg.max_batch_size, 50);
    }

    #[test]
    fn test_debounce_config_with_delay() {
        let cfg = DebounceConfig::with_delay(Duration::from_millis(200));
        assert_eq!(cfg.delay, Duration::from_millis(200));
        assert_eq!(cfg.max_batch_size, 50); // default
    }

    #[test]
    fn test_debounce_config_with_max_batch_size() {
        let cfg = DebounceConfig::with_max_batch_size(10);
        assert_eq!(cfg.max_batch_size, 10);
        assert_eq!(cfg.delay, Duration::from_millis(150)); // default
    }

    #[test]
    fn test_debouncer_empty_flush_ready_returns_none() {
        let mut d = Debouncer::new(DebounceConfig::default());
        assert!(d.flush_ready().is_none());
    }

    #[test]
    fn test_debouncer_force_flush_empty_returns_empty_batch() {
        let mut d = Debouncer::new(DebounceConfig::default());
        let batch = d.force_flush();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_debouncer_push_and_force_flush() {
        let mut d = Debouncer::new(DebounceConfig::default());
        d.push(make_event("/a/b.ets"));
        d.push(make_event("/a/c.ets"));
        let batch = d.force_flush();
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_debouncer_force_flush_clears_pending() {
        let mut d = Debouncer::new(DebounceConfig::default());
        d.push(make_event("/a/b.ets"));
        let _batch = d.force_flush();
        // After flush the buffer must be empty.
        assert!(!d.has_pending());
        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn test_debouncer_flush_ready_before_delay_returns_none() {
        // Use a very long delay so it definitely hasn't elapsed.
        let cfg = DebounceConfig::with_delay(Duration::from_secs(3600));
        let mut d = Debouncer::new(cfg);
        d.push(make_event("/a/b.ets"));
        // Delay has not elapsed → None.
        assert!(d.flush_ready().is_none());
        // But the event is still pending.
        assert!(d.has_pending());
    }

    #[test]
    fn test_debouncer_flush_ready_after_delay_returns_batch() {
        // Use zero delay so it always flushes immediately.
        let cfg = DebounceConfig::with_delay(Duration::ZERO);
        let mut d = Debouncer::new(cfg);
        d.push(make_event("/a/b.ets"));
        // With zero delay the flush should be ready.
        let batch = d
            .flush_ready()
            .expect("batch must be ready with zero delay");
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn test_debouncer_flush_ready_at_max_batch_size() {
        let cfg = DebounceConfig {
            delay: Duration::from_secs(3600), // very long delay
            max_batch_size: 3,
        };
        let mut d = Debouncer::new(cfg);
        d.push(make_event("/a/1.ets"));
        d.push(make_event("/a/2.ets"));
        assert!(d.flush_ready().is_none(), "not at max yet");
        d.push(make_event("/a/3.ets")); // hits max_batch_size
        let batch = d.flush_ready().expect("must flush at max_batch_size");
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn test_debounced_batch_collected_at_is_recent() {
        let batch = DebouncedBatch::new(vec![make_event("/a/b.ets")]);
        // collected_at should be within the last second.
        assert!(batch.collected_at.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn test_debouncer_has_pending_after_push() {
        let mut d = Debouncer::new(DebounceConfig::default());
        assert!(!d.has_pending());
        d.push(make_event("/a/b.ets"));
        assert!(d.has_pending());
        assert_eq!(d.pending_count(), 1);
    }

    #[test]
    fn test_debouncer_multiple_force_flushes() {
        let mut d = Debouncer::new(DebounceConfig::default());
        d.push(make_event("/a/b.ets"));
        let batch1 = d.force_flush();
        assert_eq!(batch1.len(), 1);

        // Second flush with no new events → empty batch.
        let batch2 = d.force_flush();
        assert!(batch2.is_empty());
    }
}
