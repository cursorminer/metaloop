use grain_looper::beats_to_samples;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use rand::Rng;
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
mod ui;
use delay_line::DelayLine;
use grain_looper::samples_to_beats;
use grain_looper::GrainLooper;
use stereo_pair::StereoPair;
use ui::waveform_display::WaveformBar;

// This is a shortened version of the gain example with most comments removed, check out
// https://github.com/robbert-vdh/nih-plug/blob/master/plugins/examples/gain/src/lib.rs to get
// started

const GUI_WIDTH: u32 = 800;
const GUI_HEIGHT: u32 = 600;
const WAVEFORM_HEIGHT: f32 = 100.0;
const XY_PAD_HEIGHT: f32 = 400.0;

struct Metaloop {
    params: Arc<MetaloopParams>,
    grain_looper: GrainLooper<StereoPair<f32>>,
    output: StereoPair<f32>,
    sample_rate: f32,
    waveform_buffer: DelayLine<WaveformBar>,
    min_sample: f32,
    max_sample: f32,
}

#[derive(Params)]
struct MetaloopParams {
    /// The parameter's ID is used to identify the parameter in the wrappred plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined.
    /// Loop length in seconds
    #[id = "loop-length"]
    pub loop_length: FloatParam,

    #[id = "length-sixteenths"]
    pub loop_length_sixteenths: IntParam,

    #[id = "loop-offset"]
    pub loop_offset: FloatParam,

    #[id = "loop-offset-sixteenths"]
    pub loop_offset_sixteenths: IntParam,

    #[id = "loop"]
    pub loop_param: BoolParam,

    #[id = "reverse"]
    pub reverse_param: BoolParam,

    #[id = "fade"]
    pub fade: FloatParam,

    #[id = "speed"]
    pub speed: FloatParam,

    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

impl Default for Metaloop {
    fn default() -> Self {
        let wave_buffer = DelayLine::new(GUI_WIDTH as usize);

        Self {
            params: Arc::new(MetaloopParams::default()),
            grain_looper: GrainLooper::new(44100.0),
            output: StereoPair::default(),
            sample_rate: 44100.0,
            waveform_buffer: wave_buffer,
            min_sample: 1.0,
            max_sample: -1.0,
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
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" s"),

            loop_length_sixteenths: IntParam::new(
                "Len 16ths",
                4,
                IntRange::Linear { min: (1), max: (8) },
            ),

            loop_offset: FloatParam::new("Offset", 0.1, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" s"),

            loop_offset_sixteenths: IntParam::new(
                "Offset 16ths",
                0,
                IntRange::Linear { min: (0), max: (7) },
            ),

            fade: FloatParam::new("Fade", 0.02, FloatRange::Linear { min: 0.0, max: 0.1 })
                .with_unit(" s"),

            speed: FloatParam::new(
                "Speed",
                100.0,
                FloatRange::Skewed {
                    min: 10.0,
                    max: 200.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" %"),

            loop_param: BoolParam::new("Loop", false),
            reverse_param: BoolParam::new("Reverse", false),
            editor_state: EguiState::from_size(GUI_WIDTH, GUI_HEIGHT),
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
        self.sample_rate = buffer_config.sample_rate as f32;

        self.grain_looper.set_sample_rate(self.sample_rate as f32);

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
        if self.grain_looper.is_looping() {
            self.update_params();
        }

        if self.grain_looper.is_looping()
            && !context.transport().playing
            && self.params.loop_param.value()
        {
            // if transport stops reset everything
            self.grain_looper.stop_looping_immediately();
            self.grain_looper.reset();
        }

        let tempo = context.transport().tempo.unwrap() as f32;
        self.grain_looper.set_tempo(tempo);

        // work out how long the UI is in samples
        let ui_width_beats = 2.0;
        let ui_width_samples = beats_to_samples(ui_width_beats, tempo, self.sample_rate);

        // we are accumulating multiple samples for each pixel
        let pixels_per_sample = GUI_WIDTH as f32 / ui_width_samples;
        let mut pixel_counter = 0.0;

        let beat_time_inc = samples_to_beats(1, tempo, self.sample_rate) as f64;
        let mut beat_time = context.transport().pos_beats().unwrap();

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
                left = false;
            }

            self.output = self.grain_looper.tick(input, beat_time);
            beat_time = beat_time + beat_time_inc;

            let mono_sample = (input.left + input.right) * 0.5;
            if mono_sample < self.min_sample {
                self.min_sample = mono_sample;
            }
            if mono_sample > self.max_sample {
                self.max_sample = mono_sample;
            }

            if pixel_counter > 1.0 {
                self.waveform_buffer.tick(WaveformBar {
                    min: self.min_sample,
                    max: self.max_sample,
                });

                self.min_sample = 1.0;
                self.max_sample = -1.0;
                pixel_counter = pixel_counter - 1.0;
            }
            pixel_counter = pixel_counter + pixels_per_sample;
        }

        // if the transport has been stopped, stop the loop and reset the block
        if !self.grain_looper.is_looping()
            && context.transport().playing
            && self.params.loop_param.value()
        {
            // if the transport has started, and loop is on then restart looping the first input
            self.grain_looper.start_looping();
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let wave = self.waveform_buffer.clone();

        let border = 4.0;
        // this is bad
        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                let screen_rect = egui_ctx.screen_rect();
                let window_size = screen_rect.size();

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.add(
                        ui::WaveformDisplay::with_wave(&wave)
                            .with_width(window_size.x - border * 2.0)
                            .with_height(WAVEFORM_HEIGHT),
                    );

                    ui.add(
                        ui::MyParamSlider::for_param(
                            &params.loop_offset_sixteenths,
                            &params.loop_length_sixteenths,
                            &params.loop_param,
                            setter,
                        )
                        .with_width(window_size.x - border * 2.0)
                        .with_height(XY_PAD_HEIGHT),
                    );
                });
            },
        )
    }
}

impl Metaloop {
    pub fn update_params(&mut self) {
        // self.grain_looper.set_grid(self.params.loop_length.value());

        self.grain_looper
            .set_grid((self.params.loop_length_sixteenths.value() as f32) / 4.0);
        self.grain_looper
            .set_loop_offset(self.params.loop_offset_sixteenths.value() as f32 / 4.0);
        self.grain_looper
            .set_reverse(self.params.reverse_param.value());

        self.grain_looper.set_fade_time(self.params.fade.value());
        self.grain_looper
            .set_speed(self.params.speed.value() / 100.0);

        if self.params.loop_param.value() && !self.grain_looper.is_looping() {
            self.grain_looper.start_looping();
        } else if !self.params.loop_param.value() && self.grain_looper.is_looping() {
            self.grain_looper.stop_looping();
        }
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
