use nih_plug::prelude::{BoolParam, FloatParam, IntParam, IntRange, Param, ParamSetter};

use nih_plug_egui::egui::{emath, vec2, CursorIcon, Response, Sense, Ui, Widget};

use emath::{Pos2, Rect};
use std::cmp::min;

// TODO these are duplicated from the nih-plug, we should probably move them to a common place
const SYNCED_RATES: [(i32, i32); 16] = [
    (1, 64),
    (1, 48),
    (1, 32),
    (1, 24),
    (1, 16),
    (1, 12),
    (1, 8),
    (1, 6),
    (3, 16),
    (1, 4),
    (5, 16),
    (1, 3),
    (3, 8),
    (1, 2),
    (3, 4),
    (1, 1),
];

pub fn grid_size_for_int_control(value: i32) -> f32 {
    let i = min(value, SYNCED_RATES.len() as i32 - 1) as usize;
    let (num, denom) = SYNCED_RATES[i];
    4.0 * num as f32 / denom as f32
}

const NUM_BEATS_X: f32 = 4.0;

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
        (NUM_BEATS_X / quantization_step).floor() as i32 + 1
    }

    // set the normalized offset value
    fn set_normalized_x(&self, normalized_x: f32) {
        let x_steps = self.num_offset_steps(self.y_param.value());
        // first quantize to the offset quantization
        let quantized_offset = (normalized_x * x_steps as f32).floor() / x_steps as f32;
        // check if value is different
        if quantized_offset != self.normalized_value_x() {
            println!("Setting offset to {}", quantized_offset * NUM_BEATS_X);
            self.setter
                .set_parameter(self.offset_param, quantized_offset * NUM_BEATS_X);
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
            for i in 0..x_steps + 2 {
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

        // draw a square on the active grid square
        let min_x = self.norm_offset_to_x(self.normalized_value_x(), response);
        let loop_len_in_beats = grid_size_for_int_control(self.y_param.value());
        let loop_len_i_pixels = loop_len_in_beats / NUM_BEATS_X * widget_size.x;
        let max_x = min_x + loop_len_i_pixels;

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

            self.set_normalized_x(1.0 - x);
            self.set_normalized_y(y);
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
