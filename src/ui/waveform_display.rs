use std::sync::{Arc, LazyLock};

use nih_plug::prelude::{BoolParam, IntParam, IntRange, Param, ParamSetter};
use nih_plug_egui::widgets::util;

use nih_plug_egui::egui::{
    self, emath, vec2, CursorIcon, Response, Sense, Stroke, TextStyle, Ui, Vec2, Widget,
};

use rand::Rng;

use crate::delay_line::DelayLine;
use emath::{Pos2, Rangef, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaveformBar {
    pub min: f32,
    pub max: f32,
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

        for x in 0..response.rect.width() as usize {
            let rand1 = rand::thread_rng().gen_range(0..20) as f32;
            let rand2 = rand::thread_rng().gen_range(0..20) as f32;
            ui.painter().vline(
                x as f32,
                emath::Rangef {
                    max: response.rect.min.y + rand1,
                    min: response.rect.max.y + rand2,
                },
                ui.visuals().widgets.active.bg_stroke,
            );
        }
    }
}

impl Widget for WaveformDisplay<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let display_width = self.display_width.unwrap_or_else(|| 200.0);

        let display_height = self.display_height.unwrap_or_else(|| 20.0);

        let mut response =
            ui.allocate_response(vec2(display_width, display_height), Sense::click_and_drag());
        self.wave_ui(ui, &mut response);

        response
    }
}
