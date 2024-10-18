use nih_plug::wrapper::vst3::vst3_sys::vst::get_green;
use nih_plug_egui::egui::{emath, vec2, Color32, Response, Sense, Stroke, Ui, Widget};

use crate::delay_line::DelayLine;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaveformBar {
    pub min: f32,
    pub max: f32,
}

pub fn scale_linear(
    input: f32,
    input_min: f32,
    input_max: f32,
    output_min: f32,
    output_max: f32,
) -> f32 {
    let input_range = input_max - input_min;
    let output_range = output_max - output_min;
    let scaled = (input - input_min) / input_range;
    output_min + (scaled * output_range)
}

#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct WaveformDisplay<'a> {
    waveform_buffer: &'a DelayLine<WaveformBar>,
    display_width: Option<f32>,
    display_height: Option<f32>,
}

impl<'a> WaveformDisplay<'a> {
    pub fn with_wave(waveform_buffer: &'a DelayLine<WaveformBar>) -> Self {
        Self {
            waveform_buffer,
            display_width: None,
            display_height: None,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.display_width = Some(width);
        self
    }
    pub fn with_height(mut self, height: f32) -> Self {
        self.display_height = Some(height);
        self
    }

    fn wave_ui(&mut self, ui: &Ui, response: &mut Response) {
        ui.painter()
            .rect_filled(response.rect, 0.0, ui.visuals().widgets.inactive.bg_fill);
        // green color
        let color = Color32::from_rgb(0, 255, 0);
        for x in 0..response.rect.width() as usize {
            let bar = self.waveform_buffer.read(x);
            let top = response.rect.min.y
                + scale_linear(bar.max, -1.0, 1.0, response.rect.min.y, response.rect.max.y);
            let bottom = response.rect.min.y
                + scale_linear(bar.min, -1.0, 1.0, response.rect.min.y, response.rect.max.y);
            ui.painter().vline(
                x as f32 + response.rect.min.x,
                emath::Rangef {
                    min: top + 1.0,
                    max: bottom,
                },
                Stroke::new(1.0, color),
            );
        }
    }
}

impl Widget for WaveformDisplay<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let display_width = self.display_width.unwrap_or_else(|| 200.0);

        let display_height = self.display_height.unwrap_or_else(|| 20.0);

        let mut response =
            ui.allocate_response(vec2(display_width, display_height), Sense::hover());
        self.wave_ui(ui, &mut response);

        response
    }
}
