// The musical grid shared by the engine, the XY pad and the waveform display:
// beat-synced loop lengths and the width of the pad/waveform window in beats.

pub const SYNCED_RATES: [(i32, i32); 7] = [
    (1, 64),
    (1, 32),
    (1, 16),
    (1, 8),
    (1, 4),
    (1, 2),
    (1, 1),
];

// the number of beats the XY pad (and the waveform window) spans horizontally
pub const NUM_BEATS_X: f32 = 4.0;

pub fn grid_size_for_int_control(value: i32) -> f32 {
    let i = (value.max(0) as usize).min(SYNCED_RATES.len() - 1);
    let (num, denom) = SYNCED_RATES[i];
    4.0 * num as f32 / denom as f32
}
