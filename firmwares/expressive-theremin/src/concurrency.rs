//! Lock-free real-time concurrency primitives for Expressive Theremin.
//!
//! Provides:
//! 1. `AtomicTunerSnapshot`: Lock-free, wait-free SeqLock for sensor->UI telemetry snapshots.
//! 2. `ConfigChannel`: Lock-free SPSC ringbuffer for UI->sensor configuration commands.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};
use expressive_core::processor::ConfigMessage;
use heapless::spsc::{Consumer, Producer, Queue};

/// Snapshot of the theremin musical state passed to the UI renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TunerSnapshot {
    pub note_name: &'static str,
    pub octave: i8,
    pub cents: f32,
    pub volume: f32,
    pub expression: f32,
    pub freq_hz: f32,
    pub is_active: bool,
}

impl Default for TunerSnapshot {
    fn default() -> Self {
        Self {
            note_name: "--",
            octave: 0,
            cents: 0.0,
            volume: 0.0,
            expression: 0.0,
            freq_hz: 0.0,
            is_active: false,
        }
    }
}

/// A wait-free, lock-free sequence lock for single-writer multi-reader telemetry snapshots.
///
/// - The real-time sensor writer NEVER blocks and NEVER loops: it performs two atomic releases
///   and a direct memory write.
/// - The UI reader checks the sequence counter and retries only if a concurrent write occurred.
pub struct AtomicTunerSnapshot {
    seq: AtomicU32,
    data: UnsafeCell<TunerSnapshot>,
}

unsafe impl Sync for AtomicTunerSnapshot {}
unsafe impl Send for AtomicTunerSnapshot {}

impl Default for AtomicTunerSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicTunerSnapshot {
    pub const fn new() -> Self {
        Self {
            seq: AtomicU32::new(0),
            data: UnsafeCell::new(TunerSnapshot {
                note_name: "--",
                octave: 0,
                cents: 0.0,
                volume: 0.0,
                expression: 0.0,
                freq_hz: 0.0,
                is_active: false,
            }),
        }
    }

    /// Real-time writer update. Wait-free (O(1)).
    #[inline]
    pub fn write(&self, snapshot: TunerSnapshot) {
        // Step 1: increment to odd number (signalling write-in-progress)
        let _ = self.seq.fetch_add(1, Ordering::Release);
        // Step 2: write data payload
        unsafe {
            *self.data.get() = snapshot;
        }
        // Step 3: increment to even number (signalling write-complete)
        let _ = self.seq.fetch_add(1, Ordering::Release);
    }

    /// Reader update. Lock-free, loops only if interrupted during write.
    #[inline]
    pub fn read(&self) -> TunerSnapshot {
        loop {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 & 1 != 0 {
                core::hint::spin_loop();
                continue;
            }
            let val = unsafe { *self.data.get() };
            let s2 = self.seq.load(Ordering::Acquire);
            if s1 == s2 {
                return val;
            }
            core::hint::spin_loop();
        }
    }
}

/// Capacity of the UI -> Sensor config ringbuffer.
pub const CONFIG_QUEUE_CAPACITY: usize = 16;

/// Create a lock-free single-producer single-consumer configuration channel.
///
/// The producer is held by the UI thread; the consumer is held by the sensor/audio task.
pub fn create_config_channel(
    queue: &'static mut Queue<ConfigMessage, CONFIG_QUEUE_CAPACITY>,
) -> (
    Producer<'static, ConfigMessage>,
    Consumer<'static, ConfigMessage>,
) {
    queue.split()
}

#[cfg(test)]
mod tests {
    use super::*;
    use expressive_core::{RootNote, Scale};

    #[test]
    fn test_atomic_snapshot_read_write() {
        let snapshot = AtomicTunerSnapshot::new();
        assert_eq!(snapshot.read().note_name, "--");

        let updated = TunerSnapshot {
            note_name: "A",
            octave: 4,
            cents: 2.5,
            volume: 0.8,
            expression: 0.6,
            freq_hz: 440.0,
            is_active: true,
        };
        snapshot.write(updated);
        assert_eq!(snapshot.read(), updated);
    }

    #[test]
    fn test_config_channel_fifo() {
        let queue: &'static mut Queue<ConfigMessage, CONFIG_QUEUE_CAPACITY> =
            Box::leak(Box::new(Queue::new()));
        let (mut tx, mut rx) = create_config_channel(queue);

        assert!(tx.enqueue(ConfigMessage::SetRootNote(RootNote::D)).is_ok());
        assert!(tx.enqueue(ConfigMessage::SetScale(Scale::Pentatonic)).is_ok());

        assert_eq!(rx.dequeue(), Some(ConfigMessage::SetRootNote(RootNote::D)));
        assert_eq!(rx.dequeue(), Some(ConfigMessage::SetScale(Scale::Pentatonic)));
        assert_eq!(rx.dequeue(), None);
    }
}
