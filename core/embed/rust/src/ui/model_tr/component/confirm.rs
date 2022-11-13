use crate::{
    time::{Duration, Instant},
    ui::{
        component::{Component, Event, EventCtx},
        display::Icon,
        event::ButtonEvent,
        geometry::Rect,
        model_tr::component::{loader::Loader, ButtonPos, LoaderMsg, LoaderStyleSheet},
    },
};

pub enum HoldToConfirmMsg {
    Confirmed,
    FailedToConfirm,
}

pub struct HoldToConfirm<T> {
    area: Rect,
    pos: ButtonPos,
    loader: Loader<T>,
    text_width: i16,
}

// TODO: support icons
// TODO: could have some icon signaling "HOLD", so that we do not need
// to write `HOLD TO CONFIRM` (which hardly fits the screen), but just `{icon}
// CONFIRM` TODO: could unite with `Button` so we can use the same features for
// both

impl<T: AsRef<str>> HoldToConfirm<T> {
    pub fn text(pos: ButtonPos, text: T, styles: LoaderStyleSheet, duration: Duration) -> Self {
        let text_width = styles.normal.font.text_width(text.as_ref());
        Self {
            area: Rect::zero(),
            pos,
            loader: Loader::text(text, styles).with_growing_duration(duration),
            text_width,
        }
    }

    pub fn icon(pos: ButtonPos, icon: Icon, styles: LoaderStyleSheet, duration: Duration) -> Self {
        let text_width = icon.width() as i16;
        Self {
            area: Rect::zero(),
            pos,
            loader: Loader::icon(icon, styles).with_growing_duration(duration),
            text_width,
        }
    }

    /// Updating the text of the component and re-placing it.
    pub fn set_text(&mut self, text: T, button_area: Rect) {
        self.text_width = self.loader.get_text_width(&text) as i16;
        self.loader.set_text(text);
        self.place(button_area);
    }

    pub fn reset(&mut self) {
        self.loader.reset();
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.loader.set_duration(duration);
    }

    pub fn get_duration(&self) -> Duration {
        self.loader.get_duration()
    }

    pub fn get_text(&self) -> &T {
        self.loader.get_text()
    }

    fn placement(&mut self, area: Rect, pos: ButtonPos) -> Rect {
        let button_width = self.text_width + 7;
        match pos {
            ButtonPos::Left => area.split_left(button_width).0,
            ButtonPos::Right => area.split_right(button_width).1,
            ButtonPos::Middle => area.split_center(button_width).1,
        }
    }
}

impl<T: AsRef<str>> Component for HoldToConfirm<T> {
    type Msg = HoldToConfirmMsg;

    fn place(&mut self, bounds: Rect) -> Rect {
        let loader_area = self.placement(bounds, self.pos);
        self.loader.place(loader_area)
    }

    fn event(&mut self, ctx: &mut EventCtx, event: Event) -> Option<Self::Msg> {
        match event {
            Event::Button(ButtonEvent::HoldStarted) => {
                self.loader.start_growing(ctx, Instant::now());
            }
            Event::Button(ButtonEvent::HoldEnded) => {
                if self.loader.is_animating() {
                    self.loader.start_shrinking(ctx, Instant::now());
                }
            }
            _ => {}
        };

        let msg = self.loader.event(ctx, event);

        if let Some(LoaderMsg::GrownCompletely) = msg {
            return Some(HoldToConfirmMsg::Confirmed);
        }
        if let Some(LoaderMsg::ShrunkCompletely) = msg {
            return Some(HoldToConfirmMsg::FailedToConfirm);
        }

        None
    }

    fn paint(&mut self) {
        self.loader.paint();
    }
}

#[cfg(feature = "ui_debug")]
impl<T: AsRef<str>> crate::trace::Trace for HoldToConfirm<T> {
    fn trace(&self, d: &mut dyn crate::trace::Tracer) {
        d.open("HoldToConfirm");
        self.loader.trace(d);
        d.close();
    }
}
