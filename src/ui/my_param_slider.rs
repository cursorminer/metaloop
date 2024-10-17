use std::sync::{Arc, LazyLock};

use nih_plug::prelude::{Param, ParamSetter};
use nih_plug_egui::widgets::util;

use nih_plug_egui::egui::{
    self, emath, vec2, CursorIcon, Response, Sense, Stroke, TextStyle, Ui, Vec2, Widget,
};

/// When shift+dragging a parameter, one pixel dragged corresponds to this much change in the
/// noramlized parameter.
const GRANULAR_DRAG_MULTIPLIER: f32 = 0.0015;

static DRAG_NORMALIZED_START_VALUE_MEMORY_ID: LazyLock<egui::Id> =
    LazyLock::new(|| egui::Id::new((file!(), 0)));
static DRAG_AMOUNT_MEMORY_ID: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::new((file!(), 1)));

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
pub struct MyParamSlider<'a, P: Param> {
    param: &'a P,
    setter: &'a ParamSetter<'a>,

    slider_width: Option<f32>,
    slider_height: Option<f32>,

    /// Will be set in the `ui()` function so we can request keyboard input focus on Alt+click.
    keyboard_focus_id: Option<egui::Id>,

    click_pos: Option<emath::Pos2>,
}

impl<'a, P: Param> MyParamSlider<'a, P> {
    /// Create a new slider for a parameter. Use the other methods to modify the slider before
    /// passing it to [`Ui::add()`].
    pub fn for_param(param: &'a P, setter: &'a ParamSetter<'a>) -> Self {
        Self {
            param,
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

    fn plain_value(&self) -> P::Plain {
        self.param.modulated_plain_value()
    }

    fn normalized_value(&self) -> f32 {
        self.param.modulated_normalized_value()
    }

    fn string_value(&self) -> String {
        self.param.to_string()
    }

    /// Enable the keyboard entry part of the widget.
    fn begin_keyboard_entry(&self, ui: &Ui) {}

    fn keyboard_entry_active(&self, ui: &Ui) -> bool {
        ui.memory(|mem| mem.has_focus(self.keyboard_focus_id.unwrap()))
    }

    fn begin_drag(&self) {
        self.setter.begin_set_parameter(self.param);
    }

    fn set_normalized_value(&self, normalized: f32) {
        // This snaps to the nearest plain value if the parameter is stepped in some way.
        // TODO: As an optimization, we could add a `const CONTINUOUS: bool` to the parameter to
        //       avoid this normalized->plain->normalized conversion for parameters that don't need
        //       it
        let value = self.param.preview_plain(normalized);
        if value != self.plain_value() {
            self.setter.set_parameter(self.param, value);
        }
    }

    /// Begin and end drag still need to be called when using this. Returns `false` if the string
    /// could no tbe parsed.
    fn set_from_string(&self, string: &str) -> bool {
        match self.param.string_to_normalized_value(string) {
            Some(normalized_value) => {
                self.set_normalized_value(normalized_value);
                true
            }
            None => false,
        }
    }

    /// Begin and end drag still need to be called when using this..
    fn reset_param(&self) {
        self.setter
            .set_parameter(self.param, self.param.default_plain_value());
    }

    fn granular_drag(&self, ui: &Ui, drag_delta: Vec2) {
        // Remember the intial position when we started with the granular drag. This value gets
        // reset whenever we have a normal itneraction with the slider.
        let start_value = if Self::get_drag_amount_memory(ui) == 0.0 {
            Self::set_drag_normalized_start_value_memory(ui, self.normalized_value());
            self.normalized_value()
        } else {
            Self::get_drag_normalized_start_value_memory(ui)
        };

        let total_drag_distance = drag_delta.x + Self::get_drag_amount_memory(ui);
        Self::set_drag_amount_memory(ui, total_drag_distance);

        self.set_normalized_value(
            (start_value + (total_drag_distance * GRANULAR_DRAG_MULTIPLIER)).clamp(0.0, 1.0),
        );
    }

    fn end_drag(&self) {
        self.setter.end_set_parameter(self.param);
    }

    fn get_drag_normalized_start_value_memory(ui: &Ui) -> f32 {
        ui.memory(|mem| {
            mem.data
                .get_temp(*DRAG_NORMALIZED_START_VALUE_MEMORY_ID)
                .unwrap_or(0.5)
        })
    }

    fn set_drag_normalized_start_value_memory(ui: &Ui, amount: f32) {
        ui.memory_mut(|mem| {
            mem.data
                .insert_temp(*DRAG_NORMALIZED_START_VALUE_MEMORY_ID, amount)
        });
    }

    fn get_drag_amount_memory(ui: &Ui) -> f32 {
        ui.memory(|mem| mem.data.get_temp(*DRAG_AMOUNT_MEMORY_ID).unwrap_or(0.0))
    }

    fn set_drag_amount_memory(ui: &Ui, amount: f32) {
        ui.memory_mut(|mem| mem.data.insert_temp(*DRAG_AMOUNT_MEMORY_ID, amount));
    }

    fn slider_ui(&mut self, ui: &Ui, response: &mut Response) {
        // Handle user input
        // TODO: Optionally (since it can be annoying) add scrolling behind a builder option
        if response.drag_started() {
            // When beginning a drag or dragging normally, reset the memory used to keep track of
            // our granular drag
            self.begin_drag();
            Self::set_drag_amount_memory(ui, 0.0);
        }
        if let Some(click_pos) = response.interact_pointer_pos() {
            if ui.input(|i| i.modifiers.command) {
                // Like double clicking, Ctrl+Click should reset the parameter
                self.reset_param();
                response.mark_changed();
            // // FIXME: This releases the focus again when you release the mouse button without
            // //        moving the mouse a bit for some reason
            // } else if ui.input().modifiers.alt && self.draw_value {
            //     // Allow typing in the value on an Alt+Click. Right now this is shown as part of the
            //     // value field, so it only makes sense when we're drawing that.
            //     self.begin_keyboard_entry(ui);
            } else if ui.input(|i| i.modifiers.shift) {
                // And shift dragging should switch to a more granulra input method
                self.granular_drag(ui, response.drag_delta());
                response.mark_changed();
            } else {
                let proportion =
                    emath::remap_clamp(click_pos.x, response.rect.x_range(), 0.0..=1.0) as f64;
                self.set_normalized_value(proportion as f32);
                response.mark_changed();
                Self::set_drag_amount_memory(ui, 0.0);
            }

            self.click_pos = response.interact_pointer_pos();
        }
        if response.double_clicked() {
            self.reset_param();
            response.mark_changed();
        }
        if response.drag_stopped() {
            self.end_drag();
        }

        // And finally draw the thing
        if ui.is_rect_visible(response.rect) {
            // We'll do a flat widget with background -> filled foreground -> slight border
            ui.painter()
                .rect_filled(response.rect, 0.0, ui.visuals().widgets.inactive.bg_fill);

            let filled_proportion = self.normalized_value();
            if filled_proportion > 0.0 {
                let mut filled_rect = response.rect;
                filled_rect.set_width(response.rect.width() * filled_proportion);
                let filled_bg = if response.dragged() {
                    util::add_hsv(ui.visuals().selection.bg_fill, 0.0, -0.1, 0.1)
                } else {
                    ui.visuals().selection.bg_fill
                };
                ui.painter().rect_filled(filled_rect, 0.0, filled_bg);
            }

            ui.painter().rect_stroke(
                response.rect,
                0.0,
                Stroke::new(1.0, ui.visuals().widgets.active.bg_fill),
            );

            // draw a dot at mouse pos for fun
            if let Some(click_pos) = self.click_pos {
                ui.painter().circle(
                    click_pos,
                    5.0,
                    ui.visuals().selection.bg_fill,
                    Stroke::new(1.0, ui.visuals().widgets.active.bg_fill),
                );
            }
        }
        // This doesn't work....
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
    }
}

impl<P: Param> Widget for MyParamSlider<'_, P> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let slider_width = self
            .slider_width
            .unwrap_or_else(|| ui.spacing().slider_width);

        let slider_height = self
            .slider_width
            .unwrap_or_else(|| ui.spacing().slider_rail_height);

        ui.horizontal(|ui| {
            // Allocate space, but add some padding on the top and bottom to make it look a bit slimmer.
            let height = ui
                .text_style_height(&TextStyle::Body)
                .max(ui.spacing().interact_size.y * 0.8);
            let mut response = ui
                .vertical(|ui| {
                    ui.allocate_space(vec2(slider_width, (height - slider_height) / 2.0));
                    let response = ui.allocate_response(
                        vec2(slider_width, slider_height),
                        Sense::click_and_drag(),
                    );
                    let (kb_edit_id, _) =
                        ui.allocate_space(vec2(slider_width, (height - slider_height) / 2.0));
                    // Allocate an automatic ID for keeping track of keyboard focus state
                    // FIXME: There doesn't seem to be a way to generate IDs in the public API, not sure how
                    //        you're supposed to do this
                    self.keyboard_focus_id = Some(kb_edit_id);

                    response
                })
                .inner;

            self.slider_ui(ui, &mut response);

            response
        })
        .inner
    }
}
