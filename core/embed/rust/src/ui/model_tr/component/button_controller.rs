use super::{
    theme, Button, ButtonDetails, ButtonLayout, ButtonPos, HoldToConfirm, HoldToConfirmMsg,
    LoaderStyleSheet,
};
use crate::{
    time::Duration,
    ui::{
        component::{base::Event, Component, EventCtx, Pad},
        event::{ButtonEvent, PhysicalButton},
        geometry::Rect,
    },
};

use heapless::String;

#[derive(Copy, Clone, PartialEq, Eq)]
enum ButtonState {
    Nothing,
    OneDown(PhysicalButton),
    BothDown,
    OneReleased(PhysicalButton),
    HTCNeedsRelease(PhysicalButton),
}

pub enum ButtonControllerMsg {
    Triggered(ButtonPos),
}

/// Defines what kind of button should be currently used.
pub enum ButtonType<T> {
    Button(Button<T>),
    HoldToConfirm(HoldToConfirm<T>),
    Nothing,
}

impl<T> ButtonType<T>
where
    T: AsRef<str>,
    T: Clone,
{
    pub fn from_button_details(pos: ButtonPos, btn_details: Option<ButtonDetails<T>>) -> Self {
        if let Some(btn_details) = btn_details {
            if btn_details.duration.is_some() {
                Self::HoldToConfirm(Self::get_hold_to_confirm(pos, btn_details))
            } else {
                Self::Button(Self::get_button(pos, btn_details))
            }
        } else {
            Self::Nothing
        }
    }

    /// Create `Button` component from `btn_details`.
    fn get_button(pos: ButtonPos, btn_details: ButtonDetails<T>) -> Button<T> {
        // Deciding between text and icon
        if let Some(text) = btn_details.clone().text {
            Button::with_text(pos, text, btn_details.style())
        } else if let Some(icon) = btn_details.icon {
            Button::with_icon(pos, icon, btn_details.style())
        } else {
            panic!("ButtonContainer: no text or icon provided");
        }
    }

    /// Create `HoldToConfirm` component from `btn_details`.
    fn get_hold_to_confirm(pos: ButtonPos, btn_details: ButtonDetails<T>) -> HoldToConfirm<T> {
        let duration = btn_details
            .duration
            .unwrap_or_else(|| Duration::from_millis(1000));
        if let Some(text) = btn_details.text {
            HoldToConfirm::text(pos, text, LoaderStyleSheet::default(), duration)
        } else if let Some(icon) = btn_details.icon {
            HoldToConfirm::icon(pos, icon, LoaderStyleSheet::default(), duration)
        } else {
            panic!("ButtonContainer: no text or icon provided");
        }
    }

    pub fn place(&mut self, button_area: Rect) {
        match self {
            Self::Button(button) => {
                button.place(button_area);
            }
            Self::HoldToConfirm(htc) => {
                htc.place(button_area);
            }
            Self::Nothing => {}
        }
    }

    pub fn paint(&mut self) {
        match self {
            Self::Button(button) => {
                button.paint();
            }
            Self::HoldToConfirm(htc) => {
                htc.paint();
            }
            Self::Nothing => {}
        }
    }
}

/// Wrapping a button and its state, so that it can be easily
/// controlled from outside.
///
/// Users have a choice of a normal button or Hold-to-confirm button.
/// `button_type` specified what from those two is used, if anything.
pub struct ButtonContainer<T> {
    pos: ButtonPos,
    button_type: ButtonType<T>,
}

impl<T: Clone + AsRef<str>> ButtonContainer<T> {
    /// Supplying `None` as `btn_details`  marks the button inactive
    /// (it can be later activated in `set()`).
    pub fn new(pos: ButtonPos, btn_details: Option<ButtonDetails<T>>) -> Self {
        Self {
            pos,
            button_type: ButtonType::from_button_details(pos, btn_details),
        }
    }

    /// Changing the state of the button.
    ///
    /// Passing `None` as `btn_details` will mark the button as inactive.
    pub fn set(&mut self, btn_details: Option<ButtonDetails<T>>, button_area: Rect) {
        self.button_type = ButtonType::from_button_details(self.pos, btn_details);
        self.button_type.place(button_area);
    }

    /// Placing the possible component.
    pub fn place(&mut self, bounds: Rect) {
        self.button_type.place(bounds);
    }

    /// Painting the component that should be currently visible, if any.
    pub fn paint(&mut self) {
        self.button_type.paint();
    }

    /// Setting the visual state of the button - released/pressed.
    pub fn set_pressed(&mut self, ctx: &mut EventCtx, is_pressed: bool) {
        if let ButtonType::Button(btn) = &mut self.button_type {
            btn.set_pressed(ctx, is_pressed);
        }
    }

    /// Whether single-click should trigger action.
    pub fn reacts_to_single_click(&self) -> bool {
        matches!(self.button_type, ButtonType::Button(_))
    }

    /// Find out whether hold-to-confirm was triggered.
    pub fn htc_got_triggered(&mut self, ctx: &mut EventCtx, event: Event) -> bool {
        if let ButtonType::HoldToConfirm(htc) = &mut self.button_type {
            if matches!(htc.event(ctx, event), Some(HoldToConfirmMsg::Confirmed)) {
                return true;
            }
        }
        false
    }

    /// Registering hold event.
    pub fn hold_started(&mut self, ctx: &mut EventCtx) {
        if let ButtonType::HoldToConfirm(htc) = &mut self.button_type {
            htc.event(ctx, Event::Button(ButtonEvent::HoldStarted));
        }
    }

    /// Cancelling hold event.
    pub fn hold_ended(&mut self, ctx: &mut EventCtx) {
        if let ButtonType::HoldToConfirm(htc) = &mut self.button_type {
            htc.event(ctx, Event::Button(ButtonEvent::HoldEnded));
        }
    }
}

/// Component responsible for handling buttons.
///
/// Acts as a state-machine of `ButtonState`.
///
/// Storing all three possible buttons - left, middle and right -
/// and handling their placement, painting and returning
/// appropriate events when they are triggered.
///
/// Buttons can be interactively changed by clients by `set()`.
///
/// Only "final" button events are returned in `ButtonControllerMsg::Triggered`,
/// based upon the buttons being long-press or not.
pub struct ButtonController<T> {
    pad: Pad,
    left_btn: ButtonContainer<T>,
    middle_btn: ButtonContainer<T>,
    right_btn: ButtonContainer<T>,
    state: ButtonState,
    // Button area is needed so the buttons
    // can be "re-placed" after their text is changed
    // Will be set in `place`
    button_area: Rect,
}

impl<T: Clone + AsRef<str>> ButtonController<T> {
    pub fn new(btn_layout: ButtonLayout<T>) -> Self {
        Self {
            pad: Pad::with_background(theme::BG).with_clear(),
            left_btn: ButtonContainer::new(ButtonPos::Left, btn_layout.btn_left),
            middle_btn: ButtonContainer::new(ButtonPos::Middle, btn_layout.btn_middle),
            right_btn: ButtonContainer::new(ButtonPos::Right, btn_layout.btn_right),
            state: ButtonState::Nothing,
            button_area: Rect::zero(),
        }
    }

    /// Updating all the three buttons to the wanted states.
    pub fn set(&mut self, btn_layout: ButtonLayout<T>) {
        self.pad.clear();
        self.left_btn.set(btn_layout.btn_left, self.button_area);
        self.middle_btn.set(btn_layout.btn_middle, self.button_area);
        self.right_btn.set(btn_layout.btn_right, self.button_area);
    }

    /// Setting the pressed state for all three buttons by boolean flags.
    fn set_pressed(&mut self, ctx: &mut EventCtx, left: bool, mid: bool, right: bool) {
        self.left_btn.set_pressed(ctx, left);
        self.middle_btn.set_pressed(ctx, mid);
        self.right_btn.set_pressed(ctx, right);
    }

    /// Handle middle button hold-to-confirm start.
    /// We need to cancel possible holds in both other buttons.
    fn middle_hold_started(&mut self, ctx: &mut EventCtx) {
        self.left_btn.hold_ended(ctx);
        self.middle_btn.hold_started(ctx);
        self.right_btn.hold_ended(ctx);
    }

    /// Handling the expiration of HTC elements.
    /// Finding out if they have been triggered and sending event
    /// for the appropriate button.
    /// Setting the state to wait for the appropriate release event
    /// from the pressed button. Also resetting visible state.
    fn handle_htc_expiration(
        &mut self,
        ctx: &mut EventCtx,
        event: Event,
    ) -> Option<ButtonControllerMsg> {
        if self.left_btn.htc_got_triggered(ctx, event) {
            self.state = ButtonState::HTCNeedsRelease(PhysicalButton::Left);
            self.set_pressed(ctx, false, false, false);
            return Some(ButtonControllerMsg::Triggered(ButtonPos::Left));
        } else if self.middle_btn.htc_got_triggered(ctx, event) {
            // TODO: how to handle it here? Do we even need to?
            self.state = ButtonState::Nothing;
            self.set_pressed(ctx, false, false, false);
            return Some(ButtonControllerMsg::Triggered(ButtonPos::Middle));
        } else if self.right_btn.htc_got_triggered(ctx, event) {
            self.state = ButtonState::HTCNeedsRelease(PhysicalButton::Right);
            self.set_pressed(ctx, false, false, false);
            return Some(ButtonControllerMsg::Triggered(ButtonPos::Right));
        }
        None
    }
}

impl<T: Clone + AsRef<str>> Component for ButtonController<T> {
    type Msg = ButtonControllerMsg;

    fn event(&mut self, ctx: &mut EventCtx, event: Event) -> Option<Self::Msg> {
        // State machine for the ButtonController
        // We are matching event with `Event::Button` for a button action
        // and `Event::Timer` for getting the expiration of HTC.
        match event {
            Event::Button(button) => {
                let (new_state, event) = match self.state {
                    ButtonState::Nothing => match button {
                        ButtonEvent::ButtonPressed(which) => {
                            match which {
                                PhysicalButton::Left => {
                                    self.left_btn.hold_started(ctx);
                                }
                                PhysicalButton::Right => {
                                    self.right_btn.hold_started(ctx);
                                }
                                _ => {}
                            }
                            (ButtonState::OneDown(which), None)
                        }
                        _ => (self.state, None),
                    },
                    ButtonState::OneDown(which_down) => match button {
                        ButtonEvent::ButtonReleased(b) if b == which_down => match which_down {
                            PhysicalButton::Left => (
                                ButtonState::Nothing,
                                if self.left_btn.reacts_to_single_click() {
                                    Some(ButtonControllerMsg::Triggered(ButtonPos::Left))
                                } else {
                                    self.left_btn.hold_ended(ctx);
                                    None
                                },
                            ),
                            PhysicalButton::Right => (
                                ButtonState::Nothing,
                                if self.right_btn.reacts_to_single_click() {
                                    Some(ButtonControllerMsg::Triggered(ButtonPos::Right))
                                } else {
                                    self.right_btn.hold_ended(ctx);
                                    None
                                },
                            ),
                            _ => (ButtonState::Nothing, None),
                        },

                        ButtonEvent::ButtonPressed(b) if b != which_down => {
                            self.middle_hold_started(ctx);
                            (ButtonState::BothDown, None)
                        }
                        _ => (self.state, None),
                    },
                    ButtonState::BothDown => match button {
                        ButtonEvent::ButtonReleased(b) => {
                            self.middle_btn.hold_ended(ctx);
                            (ButtonState::OneReleased(b), None)
                        }
                        _ => (self.state, None),
                    },
                    ButtonState::OneReleased(which_up) => match button {
                        ButtonEvent::ButtonPressed(b) if b == which_up => {
                            self.middle_hold_started(ctx);
                            (ButtonState::BothDown, None)
                        }
                        ButtonEvent::ButtonReleased(b) if b != which_up => (
                            ButtonState::Nothing,
                            if self.middle_btn.reacts_to_single_click() {
                                Some(ButtonControllerMsg::Triggered(ButtonPos::Middle))
                            } else {
                                None
                            },
                        ),
                        _ => (self.state, None),
                    },
                    ButtonState::HTCNeedsRelease(needs_release) => match button {
                        // Only going out of this state if correct button was released
                        ButtonEvent::ButtonReleased(released) if needs_release == released => {
                            (ButtonState::Nothing, None)
                        }
                        _ => (self.state, None),
                    },
                };

                // Updating the visual feedback for the buttons
                match new_state {
                    // Not showing anything also when we wait for a release
                    ButtonState::Nothing | ButtonState::HTCNeedsRelease(_) => {
                        self.set_pressed(ctx, false, false, false);
                    }
                    ButtonState::OneDown(down_button) => match down_button {
                        PhysicalButton::Left => {
                            self.set_pressed(ctx, true, false, false);
                        }
                        PhysicalButton::Right => {
                            self.set_pressed(ctx, false, false, true);
                        }
                        _ => {}
                    },
                    ButtonState::BothDown | ButtonState::OneReleased(_) => {
                        self.set_pressed(ctx, false, true, false);
                    }
                };

                self.state = new_state;
                event
            }
            Event::Timer(_) => self.handle_htc_expiration(ctx, event),
            _ => None,
        }
    }

    fn paint(&mut self) {
        self.pad.paint();
        self.left_btn.paint();
        self.middle_btn.paint();
        self.right_btn.paint();
    }

    fn place(&mut self, bounds: Rect) -> Rect {
        // Saving button area so that we can re-place the buttons
        // when they get updated
        self.button_area = bounds;

        self.pad.place(bounds);
        self.left_btn.place(bounds);
        self.middle_btn.place(bounds);
        self.right_btn.place(bounds);

        bounds
    }
}

#[cfg(feature = "ui_debug")]
use super::ButtonContent;

#[cfg(feature = "ui_debug")]
impl<T> crate::trace::Trace for ButtonContainer<T>
where
    T: AsRef<str>,
{
    fn trace(&self, t: &mut dyn crate::trace::Tracer) {
        t.open("ButtonContainer");

        // Putting together text representation of the button
        let mut btn_text: String<30> = String::new();
        if let ButtonType::Button(btn) = &self.button_type {
            match btn.content() {
                ButtonContent::Text(text) => {
                    unwrap!(btn_text.push_str(text.as_ref()));
                }
                ButtonContent::Icon(icon) => {
                    unwrap!(btn_text.push_str("Icon:"));
                    unwrap!(btn_text.push_str(icon.text));
                }
            }
        } else if let ButtonType::HoldToConfirm(htc) = &self.button_type {
            unwrap!(btn_text.push_str(htc.get_text().as_ref()));
            unwrap!(btn_text.push_str(" (HTC:"));
            unwrap!(btn_text.push_str(inttostr!(htc.get_duration().to_millis())));
            unwrap!(btn_text.push_str(")"));
        } else {
            unwrap!(btn_text.push_str(crate::trace::EMPTY_BTN));
        }
        t.button(btn_text.as_ref());

        t.close();
    }
}

#[cfg(feature = "ui_debug")]
impl<T> crate::trace::Trace for ButtonController<T>
where
    T: AsRef<str>,
{
    fn trace(&self, t: &mut dyn crate::trace::Tracer) {
        t.open("ButtonController");
        t.field("left_btn", &self.left_btn);
        t.field("middle_btn", &self.middle_btn);
        t.field("right_btn", &self.right_btn);
        t.close();
    }
}
