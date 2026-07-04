// Lock-free waveform feed from the audio thread to the GUI.
//
// The window is NUM_BEATS_X beats wide, split into WAVEFORM_BINS min/max bins.
// Bins are aligned to absolute beat time (bin k covers beats
// [k*BIN_BEATS, (k+1)*BIN_BEATS)), and BINS-per-beat is a power of two, so
// every SYNCED_RATES grid division lands exactly on bin edges - the pad's
// divisions and the waveform therefore line up by construction, at any tempo.
//
// The audio thread writes; the GUI only reads. Each bin is a single AtomicU64
// holding two packed f32s, so reads can never tear. When a loop commits, the
// live ring is copied into a snapshot ring that the GUI displays frozen until
// looping ends.

use crate::sync_rates::NUM_BEATS_X;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const WAVEFORM_BINS: usize = 512;
pub const BIN_BEATS: f64 = NUM_BEATS_X as f64 / WAVEFORM_BINS as f64;

fn pack(min: f32, max: f32) -> u64 {
    ((min.to_bits() as u64) << 32) | max.to_bits() as u64
}

fn unpack(packed: u64) -> (f32, f32) {
    (
        f32::from_bits((packed >> 32) as u32),
        f32::from_bits(packed as u32),
    )
}

pub struct WaveformState {
    // rings keyed by absolute_bin % WAVEFORM_BINS
    live: [AtomicU64; WAVEFORM_BINS],
    snapshot: [AtomicU64; WAVEFORM_BINS],
    // absolute index of the newest *complete* live bin
    newest_bin: AtomicU64,
    // absolute bin index of the loop commit boundary (exclusive window end)
    snapshot_end_bin: AtomicU64,
    frozen: AtomicBool,
}

impl WaveformState {
    pub fn new() -> WaveformState {
        WaveformState {
            live: std::array::from_fn(|_| AtomicU64::new(pack(0.0, 0.0))),
            snapshot: std::array::from_fn(|_| AtomicU64::new(pack(0.0, 0.0))),
            newest_bin: AtomicU64::new(0),
            snapshot_end_bin: AtomicU64::new(0),
            frozen: AtomicBool::new(false),
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.load(Ordering::Acquire)
    }

    // audio thread: freeze the display at a loop commit. `beat_time` is the
    // tick on which the loop committed, at most one sample past the (bin
    // aligned) grid boundary, so flooring excludes the partial current bin
    pub fn freeze(&self, beat_time: f64) {
        for (live, snapshot) in self.live.iter().zip(self.snapshot.iter()) {
            snapshot.store(live.load(Ordering::Relaxed), Ordering::Relaxed);
        }
        self.snapshot_end_bin
            .store((beat_time / BIN_BEATS) as u64, Ordering::Relaxed);
        self.frozen.store(true, Ordering::Release);
    }

    pub fn unfreeze(&self) {
        self.frozen.store(false, Ordering::Release);
    }

    fn store_bin(&self, absolute_bin: u64, min: f32, max: f32) {
        self.live[absolute_bin as usize % WAVEFORM_BINS].store(pack(min, max), Ordering::Relaxed);
        self.newest_bin.store(absolute_bin, Ordering::Release);
    }

    // GUI: the window of bins to draw, oldest first. Frozen -> the snapshot
    // ending at the commit boundary; live -> everything up to the newest
    // complete bin (right edge tracks "now")
    pub fn read_window(&self, out: &mut [(f32, f32); WAVEFORM_BINS]) {
        let (ring, end_bin) = if self.is_frozen() {
            (&self.snapshot, self.snapshot_end_bin.load(Ordering::Acquire))
        } else {
            (&self.live, self.newest_bin.load(Ordering::Acquire) + 1)
        };
        for (i, slot) in out.iter_mut().enumerate() {
            let absolute = end_bin
                .wrapping_sub(WAVEFORM_BINS as u64)
                .wrapping_add(i as u64);
            *slot = unpack(ring[absolute as usize % WAVEFORM_BINS].load(Ordering::Relaxed));
        }
    }
}

// Audio-thread accumulator for the in-progress bin. Owns no shared state so
// the binning logic is testable on its own.
pub struct WaveformWriter {
    current_bin: u64,
    min: f32,
    max: f32,
}

impl WaveformWriter {
    pub fn new() -> WaveformWriter {
        WaveformWriter {
            current_bin: 0,
            min: f32::MAX,
            max: f32::MIN,
        }
    }

    pub fn write(&mut self, state: &WaveformState, beat_time: f64, sample: f32) {
        let bin = (beat_time.max(0.0) / BIN_BEATS) as u64;
        if bin != self.current_bin {
            if self.min <= self.max {
                state.store_bin(self.current_bin, self.min, self.max);
            }
            // if the transport jumped by more than one bin, the skipped bins
            // keep their stale contents; they scroll out within one window
            self.current_bin = bin;
            self.min = f32::MAX;
            self.max = f32::MIN;
        }
        self.min = self.min.min(sample);
        self.max = self.max.max(sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_rates::{grid_size_for_int_control, SYNCED_RATES};

    #[test]
    fn test_pack_unpack_roundtrip() {
        for (min, max) in [(0.0f32, 0.0f32), (-1.0, 1.0), (-0.25, 0.75), (1e-9, -1e9)] {
            assert_eq!(unpack(pack(min, max)), (min, max));
        }
    }

    #[test]
    fn test_grid_divisions_are_bin_aligned() {
        // every selectable loop length must land exactly on bin edges,
        // otherwise the pad divisions and the waveform drift apart
        let bins_per_beat = WAVEFORM_BINS as f64 / NUM_BEATS_X as f64;
        for i in 0..SYNCED_RATES.len() {
            let beats = grid_size_for_int_control(i as i32) as f64;
            let bins = beats * bins_per_beat;
            assert_eq!(bins.fract(), 0.0, "grid {} beats = {} bins", beats, bins);
        }
    }

    #[test]
    fn test_writer_bins_by_absolute_beat_time() {
        let state = WaveformState::new();
        let mut writer = WaveformWriter::new();

        // 4 samples per bin
        let samples_per_bin = 4;
        let beat_inc = BIN_BEATS / samples_per_bin as f64;
        let mut beat_time = 0.0;
        // write two full bins plus one sample: bin 0 = 0..4, bin 1 = 40..44
        for i in 0..(2 * samples_per_bin + 1) {
            let value = if i < samples_per_bin {
                i as f32
            } else {
                (i - samples_per_bin) as f32 * 10.0
            };
            writer.write(&state, beat_time, value);
            beat_time += beat_inc;
        }

        let mut window = [(0.0, 0.0); WAVEFORM_BINS];
        state.read_window(&mut window);
        // newest complete bin (bin 1) is the last entry, bin 0 before it
        assert_eq!(window[WAVEFORM_BINS - 1], (0.0, 30.0));
        assert_eq!(window[WAVEFORM_BINS - 2], (0.0, 3.0));
    }

    #[test]
    fn test_freeze_keeps_window_while_live_advances() {
        let state = WaveformState::new();
        let mut writer = WaveformWriter::new();

        let mut beat_time = 0.0;
        // fill bins 0..=9 with a DC of 1.0 (2 samples per bin); the 21st write
        // is the first sample of bin 10 and flushes bin 9, like the sample on
        // a commit boundary does in process()
        for _ in 0..21 {
            writer.write(&state, beat_time, 1.0);
            beat_time += BIN_BEATS / 2.0;
        }
        state.freeze(beat_time);

        // keep writing different content
        for _ in 0..20 {
            writer.write(&state, beat_time, -1.0);
            beat_time += BIN_BEATS / 2.0;
        }

        let mut window = [(0.0, 0.0); WAVEFORM_BINS];
        state.read_window(&mut window);
        assert!(state.is_frozen());
        // frozen window still shows the DC 1.0 content at its right edge
        assert_eq!(window[WAVEFORM_BINS - 1], (1.0, 1.0));

        state.unfreeze();
        state.read_window(&mut window);
        // live window shows the new content
        assert_eq!(window[WAVEFORM_BINS - 1], (-1.0, -1.0));
    }
}
