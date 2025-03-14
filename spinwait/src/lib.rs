use std::hint::spin_loop;
use std::thread::yield_now;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

/// A lightweight synchronization primitive that spins for short durations before yielding.
///
/// `SpinWait` provides an adaptive spinning mechanism similar to C#’s `SpinWait` type.
/// It is designed for scenarios where a thread must wait briefly for a condition,
/// avoiding the overhead of context switching when possible.
#[derive(Debug)]
pub struct SpinWait {
    /// The number of spin iterations performed.
    count: AtomicU32,
    /// The threshold after which spinning yields to the scheduler.
    yield_threshold: u32,
}

impl SpinWait {
    /// Creates a new `SpinWait` instance with an initial count of zero.
    pub fn new() -> Self {
        SpinWait {
            count: AtomicU32::new(0),
            yield_threshold: 10,
        }
    }

    /// Creates a new `SpinWait` instance with a custom yield threshold.
    pub fn with_threshold(yield_threshold: u32) -> Self {
        Self {
            count: AtomicU32::new(0),
            yield_threshold,
        }
    }

    /// Performs a single spin iteration.
    ///
    /// If the number of iterations exceeds a threshold or the system has a single core,
    /// this method yields control to the scheduler. Otherwise, it executes a CPU spin hint.
    pub fn spin_once(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);

        // Check if we should yield based on iteration count or core count
        if self.next_spin_will_yield() || num_cpus::get() == 1 {
            yield_now();
        } else if self.count.load(Ordering::Relaxed) < 4 {
            spin_loop();
        } else {
            // Short sleep as exponential backoff
            thread::sleep(Duration::from_nanos(1 << self.count.load(Ordering::Relaxed)));
        }
    }

    /// Returns the number of spin iterations performed.
    pub fn count(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }

    /// Indicates whether the next call to `spin_once` will yield control to the scheduler.
    ///
    /// This is true if the iteration count exceeds the yield threshold or if the system
    /// has only one physical core, where spinning is less effective.
    pub fn next_spin_will_yield(&self) -> bool {
        self.count.load(Ordering::Relaxed) >= self.yield_threshold
    }

    /// Resets the spin iteration counter to zero.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
    }

    /// Spins until the provided condition returns `true`.
    ///
    /// This method repeatedly calls `spin_once` until the condition is satisfied,
    /// adapting its behavior based on the number of iterations.
    ///ß
    /// # Examples
    /// ```
    /// use spinwait::SpinWait;
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let flag = AtomicBool::new(false);
    /// let spinner = SpinWait::new();
    /// spinner.spin_until(|| flag.load(Ordering::Relaxed));
    /// ```
    pub fn spin_until<F>(&self, condition: F)
    where
        F: Fn() -> bool,
    {
        while !condition() {
            self.spin_once();
        }
    }
}

impl Default for SpinWait {
    fn default() -> Self {
        Self::new()
    }
}

/// Tests for the `SpinWait` implementation.
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    #[test]
    fn test_spin_once_increments_count() {
        let spinner = SpinWait::new();
        assert_eq!(spinner.count(), 0);
        spinner.spin_once();
        assert_eq!(spinner.count(), 1);
    }

    #[test]
    fn test_reset_clears_count() {
        let spinner = SpinWait::new();
        spinner.spin_once();
        spinner.spin_once();
        assert_eq!(spinner.count(), 2);
        spinner.reset();
        assert_eq!(spinner.count(), 0);
    }

    #[test]
    fn test_next_spin_will_yield() {
        let spinner = SpinWait::new();
        for _ in 0..spinner.yield_threshold {
            assert!(!spinner.next_spin_will_yield());
            spinner.spin_once();
        }
        assert!(spinner.next_spin_will_yield());
    }

    #[test]
    fn test_spin_until() {
        let flag = Arc::new(AtomicBool::new(false));
        let spinner = SpinWait::new();

        let handle = thread::spawn({
            let flag = flag.clone();
            move || {
                thread::sleep(std::time::Duration::from_millis(10));
                flag.store(true, Ordering::Relaxed);
            }
        });

        spinner.spin_until(|| flag.load(Ordering::Relaxed));
        assert!(flag.load(Ordering::Relaxed));
        handle.join().unwrap();
    }
}
