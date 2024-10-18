use nih_plug::prelude::{BoolParam, IntParam, IntRange, Param, ParamSetter};
use nih_plug_egui::widgets::util;

use nih_plug_egui::egui::{self, emath, vec2, CursorIcon, Response, Sense, Ui, Widget};

use emath::{Pos2, Rect};

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
    x_param: &'a IntParam,
    y_param: &'a IntParam,
    on_param: &'a BoolParam,
    setter: &'a ParamSetter<'a>,

    slider_width: Option<f32>,
    slider_height: Option<f32>,

    /// Will be set in the `ui()` function so we can request keyboard input focus on Alt+click.
    keyboard_focus_id: Option<egui::Id>,

    click_pos: Option<emath::Pos2>,
}

impl<'a> MyParamSlider<'a> {
    /// Create a new slider for a parameter. Use the other methods to modify the slider before
    /// passing it to [`Ui::add()`].
    pub fn for_param(
        x_param: &'a IntParam,
        y_param: &'a IntParam,
        on_param: &'a BoolParam,
        setter: &'a ParamSetter<'a>,
    ) -> Self {
        Self {
            x_param,
            y_param,
            on_param,

            setter,

            slider_width: None,
            slider_height: None,

            keyboard_focus_id: None,

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

    fn plain_value_x(&self) -> <nih_plug::prelude::IntParam as nih_plug::prelude::Param>::Plain {
        self.unnormalize(&self.x_param.range(), self.normalized_value_x())
    }

    fn plain_value_y(&self) -> <nih_plug::prelude::IntParam as nih_plug::prelude::Param>::Plain {
        self.unnormalize(&self.y_param.range(), self.normalized_value_y())
    }

    fn normalized_value_x(&self) -> f32 {
        self.x_param.modulated_normalized_value()
    }

    fn normalized_value_y(&self) -> f32 {
        self.y_param.modulated_normalized_value()
    }

    fn string_value_x(&self) -> String {
        self.x_param.to_string()
    }

    fn string_value_y(&self) -> String {
        self.y_param.to_string()
    }

    /// Enable the keyboard entry part of the widget.
    fn begin_keyboard_entry(&self, ui: &Ui) {}

    fn keyboard_entry_active(&self, ui: &Ui) -> bool {
        ui.memory(|mem| mem.has_focus(self.keyboard_focus_id.unwrap()))
    }

    fn begin_drag(&self) {
        self.setter.begin_set_parameter(self.x_param);
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

    fn set_normalized_value(&self, normalized_x: f32, normalized_y: f32) {
        // This snaps to the nearest plain value if the parameter is stepped in some way.
        // TODO: As an optimization, we could add a `const CONTINUOUS: bool` to the parameter to
        //       avoid this normalized->plain->normalized conversion for parameters that don't need
        //       it
        let value = self.unnormalize(&self.x_param.range(), normalized_x);
        if value != self.plain_value_x() {
            self.setter.set_parameter(self.x_param, value);
        }

        let value = self.unnormalize(&self.y_param.range(), normalized_y);
        if value != self.plain_value_y() {
            self.setter.set_parameter(self.y_param, value);
        }
    }

    /// Begin and end drag still need to be called when using this..
    fn reset_param(&self) {
        self.setter
            .set_parameter(self.x_param, self.x_param.default_plain_value());
        self.setter
            .set_parameter(self.y_param, self.y_param.default_plain_value());
    }

    fn end_drag(&self) {
        self.setter.end_set_parameter(self.x_param);
        self.setter.end_set_parameter(self.y_param);
    }

    fn slider_ui(&mut self, ui: &Ui, response: &mut Response) {
        // Handle user input
        // TODO: Optionally (since it can be annoying) add scrolling behind a builder option
        if response.drag_started() {
            // When beginning a drag or dragging normally, reset the memory used to keep track of
            // our granular drag
            self.begin_drag();
        }
        let widget_size = response.rect.size();
        if let Some(click_pos) = response.interact_pointer_pos() {
            // call set_normalized_value with normalized position

            self.set_normalized_value(click_pos.x / widget_size.x, click_pos.y / widget_size.y);

            self.click_pos = response.interact_pointer_pos();
        }
        if response.double_clicked() {
            self.reset_param();
            response.mark_changed();
        }
        if response.drag_stopped() {
            self.end_drag();
        }

        // TODO this is sending zillions of messages, stop it from doing that
        if response.is_pointer_button_down_on() {
            self.setter.begin_set_parameter(self.on_param);
            self.setter.set_parameter(self.on_param, true);
            self.setter.end_set_parameter(self.on_param);
        } else {
            self.setter.begin_set_parameter(self.on_param);
            self.setter.set_parameter(self.on_param, false);
            self.setter.end_set_parameter(self.on_param);
        }

        // And finally draw the thing
        if ui.is_rect_visible(response.rect) {
            // We'll do a flat widget with background -> filled foreground -> slight border
            ui.painter()
                .rect_filled(response.rect, 0.0, ui.visuals().widgets.inactive.bg_fill);

            // draw a grid for the steppy param
            let x_steps = self.x_param.step_count().unwrap();
            let y_steps = self.y_param.step_count().unwrap();

            let x_grid_size = widget_size.x / (x_steps + 1) as f32;
            let y_grid_size = widget_size.y / (y_steps + 1) as f32;

            for i in 0..x_steps + 2 {
                let x = i as f32 * x_grid_size + response.rect.min.x;
                ui.painter().vline(
                    x,
                    emath::Rangef {
                        max: response.rect.min.y,
                        min: response.rect.max.y,
                    },
                    ui.visuals().widgets.active.bg_stroke,
                );
            }

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
            }

            // draw a square on the active grid square
            let min_x = x_steps as f32 * self.normalized_value_x() as f32 * x_grid_size
                + response.rect.min.x;
            let max_x = min_x + x_grid_size;

            let min_y = y_steps as f32 * self.normalized_value_y() as f32 * y_grid_size
                + response.rect.min.y;
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
