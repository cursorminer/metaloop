use nih_plug::prelude::*;
use nih_plug_egui::widgets::ParamSlider;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::Arc;

mod ui;
use metaloop_dsp::grain_looper::GrainLooper;
use metaloop_dsp::stereo_pair::StereoPair;
use metaloop_dsp::sync_rates::{grid_size_for_int_control, SYNCED_RATES};
use metaloop_dsp::time_converter::TimeConverter;
use metaloop_dsp::waveform_state::{WaveformState, WaveformWriter};

const GUI_WIDTH: u32 = 800;
const GUI_HEIGHT: u32 = 600;
const WAVEFORM_HEIGHT: f32 = 100.0;
const XY_PAD_HEIGHT: f32 = 400.0;

pub struct Metaloop {
    params: Arc<MetaloopParams>,
    grain_looper: GrainLooper<StereoPair<f32>>,
    sample_rate: f32,
    waveform_state: Arc<WaveformState>,
    waveform_writer: WaveformWriter,
    loop_was_committed: bool,
}

#[derive(Params)]
struct MetaloopParams {
    /// Loop length in seconds
    #[id = "loop-length"]
    pub loop_length: FloatParam,

    #[id = "length-sixteenths"]
    pub loop_length_sixteenths: IntParam,

    #[id = "loop-offset-beats"]
    pub loop_offset_beats: FloatParam,

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
        Self {
            params: Arc::new(MetaloopParams::default()),
            grain_looper: GrainLooper::new(44100.0),
            sample_rate: 44100.0,
            waveform_state: Arc::new(WaveformState::new()),
            waveform_writer: WaveformWriter::new(),
            loop_was_committed: false,
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
                IntRange::Linear {
                    min: (0),
                    max: (SYNCED_RATES.len() as i32 - 1),
                },
            ),

            loop_offset_beats: FloatParam::new(
                "Offset",
                0.1,
                FloatRange::Linear { min: 0.0, max: 4.0 },
            )
            .with_unit(" s"),

            loop_offset_sixteenths: IntParam::new(
                "Offset 16ths",
                0,
                IntRange::Linear {
                    min: (0),
                    max: (15),
                },
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

        let tempo = context.transport().tempo.unwrap_or(120.0) as f32;
        self.grain_looper.set_tempo(tempo);

        let time = TimeConverter::new(self.sample_rate, tempo);

        let beat_time_inc = time.samples_to_beats(1) as f64;
        let beat_time_start = context.transport().pos_beats().unwrap_or(0.0);

        for (sample_idx, mut channel_samples) in buffer.iter_samples().enumerate() {
            let beat_time = beat_time_start + sample_idx as f64 * beat_time_inc;

            let input = StereoPair::new(
                *channel_samples.get_mut(0).unwrap(),
                *channel_samples.get_mut(1).unwrap(),
            );

            let output = self.grain_looper.tick(input, beat_time);

            *channel_samples.get_mut(0).unwrap() = output.left();
            *channel_samples.get_mut(1).unwrap() = output.right();

            let mono_sample = (input.left + input.right) * 0.5;
            self.waveform_writer
                .write(&self.waveform_state, beat_time, mono_sample);

            // freeze the waveform window the moment a loop commits (the first
            // grain fires at its grid boundary); unfreeze once the loop is over
            let committed = self.grain_looper.loop_committed();
            if committed && !self.loop_was_committed {
                self.waveform_state.freeze(beat_time);
            } else if !committed && self.loop_was_committed {
                self.waveform_state.unfreeze();
            }
            self.loop_was_committed = committed;
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
        let wave = self.waveform_state.clone();

        let border = 4.0;
        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                let screen_rect = egui_ctx.screen_rect();
                let window_size = screen_rect.size();

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let loop_len_beats =
                        grid_size_for_int_control(params.loop_length_sixteenths.value());

                    ui.add(
                        ui::WaveformDisplay::new(
                            wave.clone(),
                            params.loop_offset_beats.value(),
                            loop_len_beats,
                        )
                        .with_width(window_size.x - border * 2.0)
                        .with_height(WAVEFORM_HEIGHT),
                    );

                    ui.add(
                        ui::MyParamSlider::for_param(
                            &params.loop_offset_beats,
                            &params.loop_length_sixteenths,
                            &params.loop_param,
                            setter,
                        )
                        .with_width(window_size.x - border * 2.0)
                        .with_height(XY_PAD_HEIGHT),
                    );

                    ui.horizontal(|ui| {
                        let reverse_on = params.reverse_param.value();
                        if ui.selectable_label(reverse_on, "Reverse").clicked() {
                            setter.begin_set_parameter(&params.reverse_param);
                            setter.set_parameter(&params.reverse_param, !reverse_on);
                            setter.end_set_parameter(&params.reverse_param);
                        }

                        ui.label("Speed");
                        ui.add(ParamSlider::for_param(&params.speed, setter));

                        ui.label("Fade");
                        ui.add(ParamSlider::for_param(&params.fade, setter));
                    });
                });

                // the waveform scrolls with audio, not with input events, so
                // keep the editor repainting continuously
                egui_ctx.request_repaint();
            },
        )
    }
}

impl Metaloop {
    pub fn update_params(&mut self) {
        // self.grain_looper.set_grid(self.params.loop_length.value());

        self.grain_looper.set_grid(grid_size_for_int_control(
            self.params.loop_length_sixteenths.value(),
        ));
        self.grain_looper
            .set_loop_offset(self.params.loop_offset_beats.value());
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
    const CLAP_ID: &'static str = "org.cursorminer.metaloop";
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
        &[Vst3SubCategory::Fx, Vst3SubCategory::Delay];
}

nih_export_clap!(Metaloop);
nih_export_vst3!(Metaloop);
