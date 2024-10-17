use std::sync::{Arc, LazyLock};

use nih_plug::prelude::{BoolParam, IntParam, IntRange, Param, ParamSetter};
use nih_plug_egui::widgets::util;

use nih_plug_egui::egui::{
    self, emath, vec2, CursorIcon, Response, Sense, Stroke, TextStyle, Ui, Vec2, Widget,
};

use emath::{Pos2, Rangef, Rect};

/// A slider widget similar to [`egui::widgets::Slider`] that knows about NIH-plug parameters ranges
/// and can get values for it. The slider supports double click and control click to reset,
/// shift+drag for granular dragging, text value entry by clicking on the value text.
///
/// TODO: Vertical orientation
/// TODO: Check below for more input methods that should be added
/// TODO: Decouple the logic from the drawing so we can also do things like nobs without having to
///       repeat everything
/// TODO: Add WidgetInfo annotations for accessibility
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct WaveformDisplay<'a> {}

impl<'a> WaveformDisplay<'a> {
    /// Create a new slider for a parameter. Use the other methods to modify the slider before
    /// passing it to [`Ui::add()`].
    pub fn with_wave(waveform_buffer: &'a DelayLine<WaveformBar>) -> Self {
        Self {}
    }

    /// Set a custom width for the slider.
    pub fn with_width(mut self, width: f32) -> Self {
        self.slider_width = Some(width);
        self
    }
    pub fn with_height(mut self, height: f32) -> Self {
        self.slider_height = Some(height);
        self
    }

    fn slider_ui(&mut self, ui: &Ui, response: &mut Response) {}
}

impl Widget for WaveformDisplay<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.wave_ui();
    }
}
