use nih_plug::{prelude::*, wrapper::vst3::vst3_sys::vst::LegacyMidiCCOutEvent};
use std::sync::Arc;

mod countdown_trigger;
mod delay_line;
mod grain;
mod grain_looper;
mod grain_player;
mod loop_scheduler;
mod ramped_value;
mod scheduler;
mod stereo_pair;
mod test_utils;
use grain_looper::GrainLooper;
use stereo_pair::StereoPair;

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started

struct Metaloop {
    params: Arc<MetaloopParams>,
    grain_looper: GrainLooper<StereoPair<f32>>,
    output: StereoPair<f32>,
}

#[derive(Params)]
struct MetaloopParams {
    /// The parameter's ID is used to identify the parameter in the wrappred plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined.
    /// Loop length in seconds
    #[id = "loop-length"]
    pub loop_length: FloatParam,

    #[id = "loop-offset"]
    pub loop_offset: FloatParam,

    #[id = "loop"]
    pub loop_param: BoolParam,

    #[id = "reverse"]
    pub reverse_param: BoolParam,

    #[id = "fade"]
    pub fade: FloatParam,
}

impl Default for Metaloop {
    fn default() -> Self {
        Self {
            params: Arc::new(MetaloopParams::default()),
            grain_looper: GrainLooper::new(44100.0),
            output: StereoPair::default(),
        }
    }
}

impl Default for MetaloopParams {
    fn default() -> Self {
        Self {
            loop_length: FloatParam::new(
                "Length",
                0.1,
                FloatRange::Skewed {
                    min: 0.01,
                    max: 1.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s"),

            loop_offset: FloatParam::new("Offset", 0.1, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" s"),

            fade: FloatParam::new(
                "Fade",
                0.02,
                FloatRange::Skewed {
                    min: 0.005,
                    max: 0.1,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s"),

            loop_param: BoolParam::new("Loop", false),
            reverse_param: BoolParam::new("Reverse", false),
        }
    }
}

impl Plugin for Metaloop {
    const NAME: &'static str = "Metaloop";
    const VENDOR: &'static str = "Rob Tubb";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "rob@cursorminer.org";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),

        aux_input_ports: &[],
        aux_output_ports: &[],

        // Individual ports and the layout as a whole can be named here. By default these names
        // are generated as needed. This layout will be called 'Stereo', while a layout with
        // only one input and output channel would be called 'Mono'.
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.grain_looper
            .set_sample_rate(buffer_config.sample_rate as f32);

        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
        self.grain_looper.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.update_params();

        // set the tempo
        self.grain_looper
            .set_tempo(context.transport().tempo.unwrap() as f32);

        // todo: beat time only updates once per buffer
        let beat_time = context.transport().pos_beats().unwrap();

        // todo: this is utter bollocks, output will be delayed by one sample
        for channel_samples in buffer.iter_samples() {
            let _num_samples = channel_samples.len();

            let mut input: StereoPair<f32> = StereoPair::default();
            let mut left = true;

            let samples = channel_samples.into_iter();
            for sample in samples {
                if left {
                    input.left = sample.clone();
                    *sample = self.output.left();
                } else {
                    input.right = sample.clone();
                    *sample = self.output.right();
                }
            }

            self.output = self.grain_looper.tick(input, beat_time);
        }

        ProcessStatus::Normal
    }
}

impl Metaloop {
    pub fn update_params(&mut self) {
        self.grain_looper.set_grid(self.params.loop_length.value());

        if self.params.loop_param.value() && !self.grain_looper.is_looping() {
            self.grain_looper.set_loop_offset(0.1);
            self.grain_looper.start_looping();
        } else if !self.params.loop_param.value() && self.grain_looper.is_looping() {
            self.grain_looper.stop_looping();
        }
        self.grain_looper
            .set_loop_offset(self.params.loop_offset.value());
        self.grain_looper
            .set_reverse(self.params.reverse_param.value());

        self.grain_looper.set_fade_time(self.params.fade.value());
    }
}

impl ClapPlugin for Metaloop {
    const CLAP_ID: &'static str = "com.your-domain.metaloop";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A looper with scrubbing");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
}

impl Vst3Plugin for Metaloop {
    const VST3_CLASS_ID: [u8; 16] = *b"MetaMetaMetaloop";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(Metaloop);
nih_export_vst3!(Metaloop);
