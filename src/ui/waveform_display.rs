use nih_plug_egui::egui::{pos2, vec2, Color32, Rect, Response, Sense, Ui, Widget};
use std::sync::Arc;

use metaloop_dsp::sync_rates::NUM_BEATS_X;
use metaloop_dsp::waveform_state::{WaveformState, WAVEFORM_BINS};

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

// Scrolls the realtime input when not looping; freezes at the loop commit
// boundary when looping and highlights the region the current loop plays.
// The window spans the same NUM_BEATS_X beats as the XY pad below it, with
// the same x mapping (right edge = the loop-start boundary), so the pad's
// divisions line up exactly with the audio they loop.
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct WaveformDisplay {
    state: Arc<WaveformState>,
    // current loop selection, drawn as a highlight while frozen
    loop_offset_beats: f32,
    loop_len_beats: f32,
    display_width: Option<f32>,
    display_height: Option<f32>,
}

impl WaveformDisplay {
    pub fn new(
        state: Arc<WaveformState>,
        loop_offset_beats: f32,
        loop_len_beats: f32,
    ) -> Self {
        Self {
            state,
            loop_offset_beats,
            loop_len_beats,
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

    // x position of a point `beats_before_right_edge` back in time
    fn beats_to_x(&self, beats_before_right_edge: f32, rect: &Rect) -> f32 {
        rect.right() - beats_before_right_edge / NUM_BEATS_X * rect.width()
    }

    fn wave_ui(&mut self, ui: &Ui, response: &mut Response) {
        let rect = response.rect;
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().widgets.inactive.bg_fill);

        let mut window = [(0.0f32, 0.0f32); WAVEFORM_BINS];
        self.state.read_window(&mut window);

        let frozen = self.state.is_frozen();
        let color = if frozen {
            Color32::from_rgb(0, 180, 0)
        } else {
            Color32::from_rgb(0, 255, 0)
        };

        let bin_width = rect.width() / WAVEFORM_BINS as f32;
        for (i, &(min, max)) in window.iter().enumerate() {
            let x = rect.left() + i as f32 * bin_width;
            let top = scale_linear(max, 1.0, -1.0, rect.top(), rect.bottom());
            let bottom = scale_linear(min, 1.0, -1.0, rect.top(), rect.bottom());
            // at least one pixel tall so silence still draws a center line
            ui.painter().rect_filled(
                Rect {
                    min: pos2(x, top),
                    max: pos2(x + bin_width, bottom.max(top + 1.0)),
                },
                0.0,
                color,
            );
        }

        // while frozen, highlight the audio the current loop plays:
        // [offset + len, offset] beats before the commit boundary (right edge)
        if frozen {
            let left = self.beats_to_x(self.loop_offset_beats + self.loop_len_beats, &rect);
            let right = self.beats_to_x(self.loop_offset_beats, &rect);
            let highlight = Rect {
                min: pos2(left.max(rect.left()), rect.top()),
                max: pos2(right.min(rect.right()), rect.bottom()),
            };
            ui.painter().rect_filled(
                highlight,
                0.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 48),
            );
            ui.painter()
                .rect_stroke(highlight, 0.0, ui.visuals().widgets.active.fg_stroke);
        }
    }
}

impl Widget for WaveformDisplay {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let display_width = self.display_width.unwrap_or(200.0);
        let display_height = self.display_height.unwrap_or(20.0);

        let mut response =
            ui.allocate_response(vec2(display_width, display_height), Sense::hover());
        self.wave_ui(ui, &mut response);

        response
    }
}
