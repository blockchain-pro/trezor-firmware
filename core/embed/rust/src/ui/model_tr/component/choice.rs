use crate::ui::{
    component::{Child, Component, Event, EventCtx, Pad},
    geometry::Rect,
};

use super::{theme, ButtonController, ButtonControllerMsg, ButtonPos, ChoiceItem, ChoiceItemAPI};

pub enum ChoicePageMsg {
    Choice(u8),
    LeftMost,
    RightMost,
}

const MIDDLE_ROW: i32 = 72;

/// Interface for a specific component efficiently giving
/// `ChoicePage` all the information it needs to render
/// all the choice pages.
///
/// It avoids the need to store the whole sequence of
/// `ChoiceItem`s in `heapless::Vec` (which caused StackOverflow),
/// but offers a "lazy-loading" way of requesting the
/// `ChoiceItem`s only when they are needed, one-by-one.
/// This way, no more than one `ChoiceItem` is stored in memory at any time.
pub trait ChoiceFactory {
    fn get(&self, choice_index: u8) -> ChoiceItem;
    fn count(&self) -> u8;
}

/// General component displaying a set of items on the screen
/// and allowing the user to select one of them.
///
/// To be used by other more specific components that will
/// supply a set of `ChoiceItem`s (through `ChoiceFactory`)
/// and will receive back the index of the selected choice.
///
/// Each `ChoiceItem` is responsible for setting the screen -
/// choosing the button text, their duration, text displayed
/// on screen etc.
///
/// `is_carousel` can be used to make the choice page "infinite" -
/// after reaching one end, users will appear at the other end.
pub struct ChoicePage<F>
where
    F: ChoiceFactory,
{
    choices: F,
    pad: Pad,
    buttons: Child<ButtonController<&'static str>>,
    page_counter: u8,
    is_carousel: bool,
}

impl<F> ChoicePage<F>
where
    F: ChoiceFactory,
{
    pub fn new(choices: F) -> Self {
        let initial_btn_layout = choices.get(0).btn_layout();

        Self {
            choices,
            pad: Pad::with_background(theme::BG),
            buttons: Child::new(ButtonController::new(initial_btn_layout)),
            page_counter: 0,
            is_carousel: false,
        }
    }

    /// Set the page counter at the very beginning.
    pub fn with_initial_page_counter(mut self, page_counter: u8) -> Self {
        self.page_counter = page_counter;
        self
    }

    /// Enabling the carousel mode.
    pub fn with_carousel(mut self) -> Self {
        self.is_carousel = true;
        self
    }

    /// Resetting the component, which enables reusing the same instance
    /// for multiple choice categories.
    ///
    /// NOTE: from the client point of view, it would also be an option to
    /// always create a new instance with fresh setup, but I could not manage to
    /// properly clean up the previous instance - it would still be shown on
    /// screen and colliding with the new one.
    pub fn reset(
        &mut self,
        ctx: &mut EventCtx,
        new_choices: F,
        reset_page_counter: bool,
        is_carousel: bool,
    ) {
        self.choices = new_choices;
        if reset_page_counter {
            self.page_counter = 0;
        }
        self.update(ctx);
        self.is_carousel = is_carousel;
    }

    /// Navigating to the chosen page index.
    pub fn set_page_counter(&mut self, ctx: &mut EventCtx, page_counter: u8) {
        self.page_counter = page_counter;
        self.update(ctx);
    }

    /// Display current, previous and next choice according to
    /// the current ChoiceItem.
    fn paint_choices(&mut self) {
        // Performing the appropriate `paint_XXX()` for the main choice
        // and two adjacent choices when present
        // In case of carousel mode, also showing the ones from other end.
        self.show_current_choice();

        if self.has_previous_choice() {
            self.show_previous_choice();
        } else if self.is_carousel {
            self.show_last_choice_on_left();
        }

        if self.has_next_choice() {
            self.show_next_choice();
        } else if self.is_carousel {
            self.show_first_choice_on_right();
        }
    }

    /// Setting current buttons, and clearing.
    fn update(&mut self, ctx: &mut EventCtx) {
        self.set_buttons(ctx);
        self.clear(ctx);
    }

    /// Clearing the whole area and requesting repaint.
    fn clear(&mut self, ctx: &mut EventCtx) {
        self.pad.clear();
        ctx.request_paint();
    }

    fn last_page_index(&self) -> u8 {
        self.choices.count() as u8 - 1
    }

    pub fn has_previous_choice(&self) -> bool {
        self.page_counter > 0
    }

    pub fn has_next_choice(&self) -> bool {
        self.page_counter < self.last_page_index()
    }

    fn current_choice(&self) -> ChoiceItem {
        self.get_choice(self.page_counter)
    }

    fn get_choice(&self, index: u8) -> ChoiceItem {
        self.choices.get(index)
    }

    fn show_current_choice(&self) {
        self.current_choice().paint_center();
    }

    fn show_previous_choice(&self) {
        self.get_choice(self.page_counter - 1).paint_left();
    }

    fn show_next_choice(&self) {
        self.get_choice(self.page_counter + 1).paint_right();
    }

    fn show_last_choice_on_left(&self) {
        self.get_choice(self.last_page_index()).paint_left();
    }

    fn show_first_choice_on_right(&self) {
        self.get_choice(0).paint_right();
    }

    fn decrease_page_counter(&mut self) {
        self.page_counter -= 1;
    }

    fn increase_page_counter(&mut self) {
        self.page_counter += 1;
    }

    fn page_counter_to_zero(&mut self) {
        self.page_counter = 0;
    }

    fn page_counter_to_max(&mut self) {
        self.page_counter = self.last_page_index();
    }

    pub fn page_index(&self) -> u8 {
        self.page_counter
    }

    /// Updating the visual state of the buttons after each event.
    /// All three buttons are handled based upon the current choice.
    /// If defined in the current choice, setting their text,
    /// whether they are long-pressed, and painting them.
    ///
    /// NOTE: ButtonController is handling the painting, and
    /// it will not repaint the buttons unless some of them changed.
    fn set_buttons(&mut self, ctx: &mut EventCtx) {
        // TODO: offer the possibility to change the buttons from the client
        // (button details could be changed in the same index)
        // Use-case: BIN button in PIN is deleting last digit if the PIN is not empty,
        // otherwise causing Cancel. Would be nice to allow deleting as a single click
        // and Cancel as HTC. PIN client would check if the PIN is empty/not and
        // adjust the HTC/not.

        let btn_layout = self.current_choice().btn_layout();
        self.buttons.mutate(ctx, |_ctx, buttons| {
            buttons.set(btn_layout);
        });
    }
}

impl<F> Component for ChoicePage<F>
where
    F: ChoiceFactory,
{
    type Msg = ChoicePageMsg;

    fn place(&mut self, bounds: Rect) -> Rect {
        let (content_area, button_area) = bounds.split_bottom(theme::BUTTON_HEIGHT);
        self.pad.place(content_area);
        self.buttons.place(button_area);
        bounds
    }

    fn event(&mut self, ctx: &mut EventCtx, event: Event) -> Option<Self::Msg> {
        let button_event = self.buttons.event(ctx, event);

        if let Some(ButtonControllerMsg::Triggered(pos)) = button_event {
            match pos {
                ButtonPos::Left => {
                    if self.has_previous_choice() {
                        // Clicked BACK. Decrease the page counter.
                        self.decrease_page_counter();
                        self.update(ctx);
                    } else if self.is_carousel {
                        // In case of carousel going to the right end.
                        self.page_counter_to_max();
                        self.update(ctx);
                    } else {
                        // Triggered LEFTmost button. Send event
                        self.clear(ctx);
                        return Some(ChoicePageMsg::LeftMost);
                    }
                }
                ButtonPos::Right => {
                    if self.has_next_choice() {
                        // Clicked NEXT. Increase the page counter.
                        self.increase_page_counter();
                        self.update(ctx);
                    } else if self.is_carousel {
                        // In case of carousel going to the left end.
                        self.page_counter_to_zero();
                        self.update(ctx);
                    } else {
                        // Triggered RIGHTmost button. Send event
                        self.clear(ctx);
                        return Some(ChoicePageMsg::RightMost);
                    }
                }
                ButtonPos::Middle => {
                    // Clicked SELECT. Send current choice index
                    self.clear(ctx);
                    return Some(ChoicePageMsg::Choice(self.page_counter));
                }
            }
        };
        None
    }

    fn paint(&mut self) {
        self.pad.paint();
        self.buttons.paint();
        self.paint_choices();
    }
}

#[cfg(feature = "ui_debug")]
impl<F> crate::trace::Trace for ChoicePage<F>
where
    F: ChoiceFactory,
{
    fn trace(&self, t: &mut dyn crate::trace::Tracer) {
        t.open("ChoicePage");
        t.kw_pair("active_page", inttostr!(self.page_counter));
        t.kw_pair("page_count", inttostr!(self.choices.count() as u8));
        t.kw_pair("is_carousel", booltostr!(self.is_carousel));

        if self.has_previous_choice() {
            t.field("prev_choice", &self.get_choice(self.page_counter - 1));
        } else if self.is_carousel {
            // In case of carousel going to the left end.
            t.field("prev_choice", &self.get_choice(self.last_page_index()));
        } else {
            t.string("prev_choice");
            t.symbol("None");
        }

        t.field("current_choice", &self.current_choice());

        if self.has_next_choice() {
            t.field("next_choice", &self.get_choice(self.page_counter + 1));
        } else if self.is_carousel {
            // In case of carousel going to the very left.
            t.field("next_choice", &self.get_choice(0));
        } else {
            t.string("next_choice");
            t.symbol("None");
        }

        t.field("buttons", &self.buttons);
        t.close();
    }
}
