use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::Arc;

mod delay_line;
mod grain;
mod grain_looper;
mod grain_player;
mod ramped_value;
mod stereo_pair;

use crate::grain_looper::GrainLooper;

pub struct Metaloop {
    params: Arc<MetaloopParams>,

    grain_looper: GrainLooper,
}

#[derive(Params)]
pub struct MetaloopParams {
    /// The editor state, saved together with the parameter state so the custom scaling can be
    /// restored.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    /// Loop length in seconds
    #[id = "loop-length"]
    pub loop_length: FloatParam,

    #[id = "loop"]
    pub loop_param: BoolParam,
}

impl Default for Metaloop {
    fn default() -> Self {
        Self {
            params: Arc::new(MetaloopParams::default()),

            grain_looper: GrainLooper::new(44100.0),
        }
    }
}

impl Default for MetaloopParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(300, 180),

            // See the main gain example for more details
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

            loop_param: BoolParam::new("Loop", false),
        }
    }
}

impl Plugin for Metaloop {
    const NAME: &'static str = "Metaloop";
    const VENDOR: &'static str = "Minersound";
    const URL: &'static str = "";
    const EMAIL: &'static str = "rob@cursorminer.org";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    // NOTE: See `plugins/diopser/src/editor.rs` for an example using the generic UI widget

                    ui.label("Loop Length");
                    ui.add(widgets::ParamSlider::for_param(&params.loop_length, setter));

                    // add a button to toggle the loop
                    //ui.add(widgets::ParamWidget::for_param(&params.loop_length, setter));
                });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.grain_looper
            .set_sample_rate(buffer_config.sample_rate as f32);

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for channel_samples in buffer.iter_samples() {
            let num_samples = channel_samples.len();

            self.grain_looper
                .set_loop_duration(self.params.loop_length.value());

            if self.params.loop_param.value() && !self.grain_looper.is_looping() {
                self.grain_looper.set_loop_offset(0.1);
                self.grain_looper.start_looping(0.01);
            } else if !self.params.loop_param.value() && self.grain_looper.is_looping() {
                self.grain_looper.stop_looping();
            }

            let mut left = true;
            for sample in channel_samples {
                if left {
                    *sample = self.grain_looper.tick(sample.clone());
                }
                left = false;
            }

            // To save resources, a plugin can (and probably should!) only perform expensive
            // calculations that are only displayed on the GUI while the GUI is open
            if self.params.editor_state.is_open() {
                // do some animation stuff
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Metaloop {
    const CLAP_ID: &'static str = "com.minersound.metaloop";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A scrubber / looper");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Glitch,
    ];
}

impl Vst3Plugin for Metaloop {
    const VST3_CLASS_ID: [u8; 16] = *b"MetaloopYeahBoyy";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(Metaloop);
nih_export_vst3!(Metaloop);
