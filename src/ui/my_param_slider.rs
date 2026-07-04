use nih_plug::prelude::{BoolParam, FloatParam, IntParam, IntRange, Param, ParamSetter};

use nih_plug_egui::egui::{emath, vec2, CursorIcon, Response, Sense, Ui, Widget};

use metaloop_dsp::sync_rates::{grid_size_for_int_control, NUM_BEATS_X};
use emath::{Pos2, Rect};

// Map a click to the offset param value such that the clicked cell loops
// exactly the audio beneath it. The engine plays [offset + len, offset] beats
// before the loop-start boundary (offset measures how far back the loop
// *ends*), and the window's right edge is that boundary, so a cell's offset
// is the distance from the right edge to the cell's own right edge - one
// cell less than the naive left-edge mapping.
fn quantized_cell_offset_beats(normalized_x_from_left: f32, loop_len_beats: f32) -> f32 {
    let steps = (NUM_BEATS_X / loop_len_beats).floor();
    let quantized_x = (normalized_x_from_left.clamp(0.0, 1.0) * steps).floor() / steps;
    (((1.0 - quantized_x) * NUM_BEATS_X) - loop_len_beats).max(0.0)
}

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
pub struct MyParamSlider<'a> {
    offset_param: &'a FloatParam,
    y_param: &'a IntParam,
    on_param: &'a BoolParam,
    setter: &'a ParamSetter<'a>,

    slider_width: Option<f32>,
    slider_height: Option<f32>,

    click_pos: Option<emath::Pos2>,
}

impl<'a> MyParamSlider<'a> {
    /// Create a new slider for a parameter. Use the other methods to modify the slider before
    /// passing it to [`Ui::add()`].
    pub fn for_param(
        offset_param: &'a FloatParam, // this is in beats
        y_param: &'a IntParam,
        on_param: &'a BoolParam,
        setter: &'a ParamSetter<'a>,
    ) -> Self {
        Self {
            offset_param,
            y_param,
            on_param,

            setter,

            slider_width: None,
            slider_height: None,

            click_pos: None,
        }
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

    fn plain_value_y(&self) -> <nih_plug::prelude::IntParam as nih_plug::prelude::Param>::Plain {
        self.unnormalize(&self.y_param.range(), self.normalized_value_y())
    }

    fn normalized_value_x(&self) -> f32 {
        self.offset_param.modulated_normalized_value()
    }

    fn normalized_value_y(&self) -> f32 {
        self.y_param.modulated_normalized_value()
    }

    fn begin_drag(&self) {
        self.setter.begin_set_parameter(self.offset_param);
        self.setter.begin_set_parameter(self.y_param);
    }

    // this is a hack to work around the rounding in the nih-plug which we'd rather be flooring
    pub fn unnormalize(&self, &range: &IntRange, normalized: f32) -> i32 {
        let normalized = normalized.clamp(0.0, 1.0);
        match range {
            IntRange::Linear { min, max } => {
                (normalized * (max - min + 1) as f32).floor() as i32 + min
            }
            IntRange::Reversed(range) => range.unnormalize(1.0 - normalized),
        }
    }

    fn offset_quant_for_loop_length(&self, loop_length: i32) -> f32 {
        // what we will snap the offset to, given how long the loop length is
        // if less than a 16th snap to the loop lenth to allow scrubbing
        grid_size_for_int_control(loop_length)
    }

    fn num_offset_steps(&self, y_index: i32) -> i32 {
        let quantization_step = self.offset_quant_for_loop_length(y_index);
        (NUM_BEATS_X / quantization_step).floor() as i32
    }

    // set the offset param, given the mouse's normalized position from the left
    fn set_normalized_x(&self, normalized_x_from_left: f32) {
        let loop_len_beats = grid_size_for_int_control(self.y_param.value());
        let offset_beats = quantized_cell_offset_beats(normalized_x_from_left, loop_len_beats);
        // check if value is different
        if offset_beats != self.normalized_value_x() * NUM_BEATS_X {
            self.setter.set_parameter(self.offset_param, offset_beats);
        }
    }

    fn set_normalized_y(&self, normalized_y: f32) {
        let value = self.unnormalize(&self.y_param.range(), normalized_y);
        if value != self.plain_value_y() {
            self.setter.set_parameter(self.y_param, value);
        }
    }

    /// Begin and end drag still need to be called when using this..
    fn reset_param(&self) {
        self.setter
            .set_parameter(self.offset_param, self.offset_param.default_plain_value());
        self.setter
            .set_parameter(self.y_param, self.y_param.default_plain_value());
    }

    fn end_drag(&self) {
        self.setter.end_set_parameter(self.offset_param);
        self.setter.end_set_parameter(self.y_param);
    }

    // For a given normalized offset, return the quantized x position on the grid
    fn norm_offset_to_x(&self, norm_offset: f32, response: &Response) -> f32 {
        (1.0 - norm_offset) * response.rect.size().x + response.rect.min.x
    }

    fn draw_grid(&self, ui: &Ui, response: &Response) {
        let widget_size = response.rect.size();

        let y_steps = self.y_param.step_count().unwrap();

        let y_grid_size = widget_size.y / (y_steps + 1) as f32;

        for i in 0..y_steps + 2 {
            let y = i as f32 * y_grid_size + response.rect.min.y;
            ui.painter().hline(
                emath::Rangef {
                    max: response.rect.min.x,
                    min: response.rect.max.x,
                },
                y,
                ui.visuals().widgets.active.bg_stroke,
            );

            let x_steps = self.num_offset_steps(i as i32);
            let x_grid_size = widget_size.x / x_steps as f32;
            for i in 0..x_steps + 1 {
                // draw a grid for the steppy param
                let x = i as f32 * x_grid_size + response.rect.min.x;
                ui.painter().vline(
                    x,
                    emath::Rangef {
                        max: y + y_grid_size,
                        min: y,
                    },
                    ui.visuals().widgets.active.bg_stroke,
                );
            }
        }

        // draw a square on the active grid square: the loop *ends* offset
        // beats before the right edge and extends one loop length further
        // back, so the cell sits to the left of the offset point
        let max_x = self.norm_offset_to_x(self.normalized_value_x(), response);
        let loop_len_in_beats = grid_size_for_int_control(self.y_param.value());
        let loop_len_i_pixels = loop_len_in_beats / NUM_BEATS_X * widget_size.x;
        let min_x = max_x - loop_len_i_pixels;

        let min_y =
            y_steps as f32 * self.normalized_value_y() as f32 * y_grid_size + response.rect.min.y;
        let max_y = min_y + y_grid_size;

        if let Some(_) = self.click_pos {
            ui.painter().rect_filled(
                Rect {
                    min: Pos2 { x: min_x, y: min_y },
                    max: Pos2 { x: max_x, y: max_y },
                },
                0.0,
                ui.visuals().hyperlink_color,
            );
        }
    }

    fn normalized_position(&self, click_pos: Pos2, response: &Response) -> [f32; 2] {
        let widget_size = response.rect.size();
        let x = (click_pos.x - response.rect.min.x) / widget_size.x;
        let y = (click_pos.y - response.rect.min.y) / widget_size.y;
        [x, y]
    }

    fn handle_mouse_input(&mut self, response: &mut Response) {
        // Handle user input
        // TODO: Optionally (since it can be annoying) add scrolling behind a builder option
        if response.drag_started() {
            // When beginning a drag or dragging normally, reset the memory used to keep track of
            // our granular drag
            self.begin_drag();
        }
        if let Some(click_pos) = response.interact_pointer_pos() {
            // call set_normalized_value with normalized position

            let [x, y] = self.normalized_position(click_pos, response);

            self.set_normalized_y(y);
            self.set_normalized_x(x);
            self.click_pos = response.interact_pointer_pos();
        }
        if response.double_clicked() {
            self.reset_param();
            response.mark_changed();
        }
        if response.drag_stopped() {
            self.end_drag();
        }

        if response.is_pointer_button_down_on() && !self.on_param.value() {
            self.setter.begin_set_parameter(self.on_param);
            self.setter.set_parameter(self.on_param, true);
            self.setter.end_set_parameter(self.on_param);
        } else if !response.is_pointer_button_down_on() && self.on_param.value() {
            self.setter.begin_set_parameter(self.on_param);
            self.setter.set_parameter(self.on_param, false);
            self.setter.end_set_parameter(self.on_param);
        }
    }

    fn slider_ui(&mut self, ui: &Ui, response: &mut Response) {
        self.handle_mouse_input(response);

        // And finally draw the thing
        if ui.is_rect_visible(response.rect) {
            // We'll do a flat widget with background -> filled foreground -> slight border
            ui.painter()
                .rect_filled(response.rect, 0.0, ui.visuals().widgets.inactive.bg_fill);

            self.draw_grid(ui, response);
        }
        // This doesn't work....
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
    }
}

impl Widget for MyParamSlider<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let slider_width = self
            .slider_width
            .unwrap_or_else(|| ui.spacing().slider_width);

        let slider_height = self
            .slider_height
            .unwrap_or_else(|| ui.spacing().slider_rail_height);

        ui.horizontal(|ui| {
            let mut response =
                ui.allocate_response(vec2(slider_width, slider_height), Sense::click_and_drag());

            self.slider_ui(ui, &mut response);

            response
        })
        .inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantized_cell_offset_maps_cell_to_its_own_audio() {
        // for every row: clicking cell k (of n) must yield offset (n-1-k)*L,
        // so the span the engine plays, [offset + L, offset] beats before the
        // boundary, is exactly cell k's own span
        for row in 0..7 {
            let len = grid_size_for_int_control(row);
            let n = (NUM_BEATS_X / len) as i32;
            for k in 0..n {
                // click in the middle of cell k
                let x = (k as f32 + 0.5) / n as f32;
                let offset = quantized_cell_offset_beats(x, len);
                assert_eq!(offset, (n - 1 - k) as f32 * len, "row {} cell {}", row, k);

                // cell k's span in beats-before-boundary coordinates
                let cell_left = NUM_BEATS_X - k as f32 * len;
                let cell_right = NUM_BEATS_X - (k + 1) as f32 * len;
                assert_eq!(offset + len, cell_left);
                assert_eq!(offset, cell_right);
            }
        }

        // the rightmost cell reaches the most recent audio (offset 0)
        assert_eq!(quantized_cell_offset_beats(0.999, 1.0), 0.0);
    }
}
