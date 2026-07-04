//! Pure-Rust DSP core for Metaloop.
//!
//! This crate contains no nih-plug (or any plugin-framework) dependency, so
//! it can be reused on other hosts/targets (e.g. an iOS/AUv3 build) without
//! pulling in VST3/CLAP machinery. The plugin shell (VST3/CLAP/standalone)
//! lives in the top-level `metaloop` crate and depends on this one.

pub mod delay_line;
pub mod grain;
pub mod grain_looper;
pub mod grain_player;
pub mod loop_scheduler;
pub mod ramped_value;
pub mod stereo_pair;
pub mod sync_rates;
pub mod test_utils;
pub mod time_converter;
pub mod waveform_state;

pub use grain::Grain;
pub use grain_looper::GrainLooper;
pub use grain_player::GrainPlayer;
pub use stereo_pair::{AudioSampleOps, StereoPair};
pub use sync_rates::{grid_size_for_int_control, NUM_BEATS_X, SYNCED_RATES};
pub use time_converter::TimeConverter;
pub use waveform_state::{WaveformState, WaveformWriter, WAVEFORM_BINS};
