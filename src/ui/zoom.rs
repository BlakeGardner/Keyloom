//! Zooming the deck: the levels that Ctrl+scroll, Ctrl with +, −, and
//! 0, and the menu's stepper move through, what a level means in
//! pixels, and the wrapper that turns Ctrl+scroll over the deck into
//! steps while plain scrolling still reaches the scrollables around it.

use cosmic::iced::core::widget::{Operation, Tree, tree};
use cosmic::iced::core::{
    Clipboard, Element, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget, layout,
    mouse, overlay, renderer,
};
use cosmic::iced::keyboard::{self, Key, Modifiers};

use crate::config::DeckZoom;

/// The deck's zoom levels, as percentages of its natural size.
pub const DECK_LEVELS: [u16; 11] = [50, 60, 70, 80, 90, 100, 110, 125, 150, 175, 200];

/// A move through the levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// One level larger.
    In,
    /// One level smaller.
    Out,
    /// Back to the deck fitted to the window.
    Reset,
}

/// The level one step from `percent` along `levels`, or `None` past
/// their end. `percent` need not be a level itself: a fitted deck
/// steps from whatever fitting amounts to.
fn next_level(levels: &[u16], percent: f32, step: Step) -> Option<u16> {
    // Half a percent of slack keeps float noise from repeating a level.
    match step {
        Step::In => levels
            .iter()
            .copied()
            .find(|&level| f32::from(level) > percent + 0.5),
        Step::Out => levels
            .iter()
            .rev()
            .copied()
            .find(|&level| f32::from(level) < percent - 0.5),
        Step::Reset => None,
    }
}

/// The scale that fits a deck of `natural_width` into `available_width`
/// logical pixels: no larger than its natural size, and no smaller
/// than the smallest zoom level, where the legends stop being legible.
pub fn fit_factor(natural_width: f32, available_width: f32) -> f32 {
    if natural_width <= 0.0 {
        return 1.0;
    }
    let smallest = f32::from(DECK_LEVELS[0]) / 100.0;
    (available_width / natural_width).clamp(smallest, 1.0)
}

impl DeckZoom {
    /// The zoom as a scale of the deck's natural size, for a deck
    /// `natural_width` wide with `available_width` to draw in.
    pub fn factor(self, natural_width: f32, available_width: f32) -> f32 {
        match self {
            Self::Fit => fit_factor(natural_width, available_width),
            Self::Percent(percent) => f32::from(percent) / 100.0,
        }
    }

    /// A stored level brought within the levels offered.
    pub fn clamped(self) -> Self {
        match self {
            Self::Fit => Self::Fit,
            Self::Percent(percent) => {
                Self::Percent(percent.clamp(DECK_LEVELS[0], DECK_LEVELS[DECK_LEVELS.len() - 1]))
            }
        }
    }

    /// One step from this level; `fit_percent` is what fitting the
    /// deck currently amounts to, where a step from `Fit` starts.
    pub fn stepped(self, step: Step, fit_percent: f32) -> Self {
        match step {
            Step::Reset => Self::Fit,
            Step::In | Step::Out => {
                let from = match self {
                    Self::Fit => fit_percent,
                    Self::Percent(percent) => f32::from(percent),
                };
                next_level(&DECK_LEVELS, from, step).map_or(self, Self::Percent)
            }
        }
    }

    /// Whether a step changes anything: "+" at the largest level and
    /// "−" at the smallest do not.
    pub fn can_step(self, step: Step, fit_percent: f32) -> bool {
        self.stepped(step, fit_percent) != self
    }

    /// The level as the menu names it.
    pub fn label(self) -> String {
        match self {
            Self::Fit => "Fit".to_owned(),
            Self::Percent(percent) => format!("{percent}%"),
        }
    }
}

/// The deck zoom step a key press with Ctrl held asks for: + or =
/// zooms in, − out, and 0 fits the deck to the window again. The key
/// is taken as pressed and as the layout modifies it, so Ctrl+Shift+=
/// counts as the + it types.
pub fn shortcut(key: &Key, modified_key: &Key) -> Option<Step> {
    [key, modified_key].into_iter().find_map(|key| match key {
        Key::Character(character) => match character.as_str() {
            "+" | "=" => Some(Step::In),
            "-" | "_" => Some(Step::Out),
            "0" => Some(Step::Reset),
            _ => None,
        },
        Key::Named(_) | Key::Unidentified => None,
    })
}

/// How far a touchpad scrolls for one step: about a wheel's notch.
const PIXELS_PER_NOTCH: f32 = 30.0;

/// Wraps the deck so that Ctrl+scroll over it steps the zoom. Every
/// other scroll passes through to the scrollables around it, and the
/// modifiers are tracked here, from the events every widget receives,
/// so the deck needs nothing from the application to know Ctrl is held.
pub struct Area<'a, Message> {
    content: Element<'a, Message, cosmic::Theme, cosmic::Renderer>,
    on_step: fn(Step) -> Message,
}

/// [`Area`] around `content`, producing `on_step` messages.
pub fn area<'a, Message>(
    content: impl Into<Element<'a, Message, cosmic::Theme, cosmic::Renderer>>,
    on_step: fn(Step) -> Message,
) -> Area<'a, Message> {
    Area {
        content: content.into(),
        on_step,
    }
}

/// What the wrapper remembers between events.
#[derive(Default)]
struct State {
    modifiers: Modifiers,
    /// Scrolling with Ctrl held that has not added up to a step yet,
    /// in notches.
    wheel: f32,
}

impl<Message> Widget<Message, cosmic::Theme, cosmic::Renderer> for Area<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &cosmic::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &cosmic::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
                if !modifiers.control() {
                    state.wheel = 0.0;
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if !shell.is_event_captured()
                    && state.modifiers.control()
                    && cursor.is_over(layout.bounds()) =>
            {
                state.wheel += match *delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / PIXELS_PER_NOTCH,
                };
                // Whole notches become steps, scrolling up zooming in;
                // the remainder waits for the next event.
                while state.wheel >= 1.0 {
                    shell.publish((self.on_step)(Step::In));
                    state.wheel -= 1.0;
                }
                while state.wheel <= -1.0 {
                    shell.publish((self.on_step)(Step::Out));
                    state.wheel += 1.0;
                }
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &cosmic::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut cosmic::Renderer,
        theme: &cosmic::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &cosmic::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, cosmic::Theme, cosmic::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<Area<'a, Message>>
    for Element<'a, Message, cosmic::Theme, cosmic::Renderer>
{
    fn from(area: Area<'a, Message>) -> Self {
        Element::new(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::keyboard::key::Named;

    #[test]
    fn levels_step_from_on_and_off_the_ladder() {
        assert_eq!(next_level(&DECK_LEVELS, 100.0, Step::In), Some(110));
        assert_eq!(next_level(&DECK_LEVELS, 100.0, Step::Out), Some(90));
        // A fitted deck at 83% steps to the levels around it.
        assert_eq!(next_level(&DECK_LEVELS, 83.2, Step::In), Some(90));
        assert_eq!(next_level(&DECK_LEVELS, 83.2, Step::Out), Some(80));
        // Nothing past either end.
        assert_eq!(next_level(&DECK_LEVELS, 200.0, Step::In), None);
        assert_eq!(next_level(&DECK_LEVELS, 50.0, Step::Out), None);
        // Float noise does not repeat a level.
        assert_eq!(next_level(&DECK_LEVELS, 109.999, Step::In), Some(125));
        assert_eq!(next_level(&DECK_LEVELS, 90.001, Step::Out), Some(80));
    }

    #[test]
    fn fitting_shrinks_only_as_far_as_needed_and_never_past_legible() {
        let natural = 1000.0;
        assert!((fit_factor(natural, 1600.0) - 1.0).abs() < f32::EPSILON);
        assert!((fit_factor(natural, 1000.0) - 1.0).abs() < f32::EPSILON);
        assert!((fit_factor(natural, 800.0) - 0.8).abs() < 1e-6);
        assert!((fit_factor(natural, 100.0) - 0.5).abs() < f32::EPSILON);
        assert!((fit_factor(0.0, 100.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn deck_zoom_steps_from_what_fitting_amounts_to() {
        let fit = DeckZoom::Fit;
        assert_eq!(fit.stepped(Step::In, 83.0), DeckZoom::Percent(90));
        assert_eq!(fit.stepped(Step::Out, 83.0), DeckZoom::Percent(80));
        assert_eq!(fit.stepped(Step::In, 100.0), DeckZoom::Percent(110));
        assert_eq!(
            DeckZoom::Percent(125).stepped(Step::Reset, 60.0),
            DeckZoom::Fit
        );
        assert_eq!(
            DeckZoom::Percent(200).stepped(Step::In, 60.0),
            DeckZoom::Percent(200)
        );
        assert!(!DeckZoom::Percent(200).can_step(Step::In, 60.0));
        assert!(DeckZoom::Percent(200).can_step(Step::Out, 60.0));
        assert!(!DeckZoom::Fit.can_step(Step::Reset, 60.0));
    }

    #[test]
    fn stored_levels_are_brought_within_the_ladder() {
        assert_eq!(DeckZoom::Percent(5).clamped(), DeckZoom::Percent(50));
        assert_eq!(DeckZoom::Percent(999).clamped(), DeckZoom::Percent(200));
        assert_eq!(DeckZoom::Percent(125).clamped(), DeckZoom::Percent(125));
    }

    #[test]
    fn levels_are_named_for_the_menu() {
        assert_eq!(DeckZoom::Fit.label(), "Fit");
        assert_eq!(DeckZoom::Percent(90).label(), "90%");
        assert!((DeckZoom::Percent(125).factor(1000.0, 100.0) - 1.25).abs() < f32::EPSILON);
    }

    #[test]
    fn shortcuts_read_the_key_as_pressed_or_as_the_layout_types_it() {
        let character = |text: &str| Key::Character(text.into());
        assert_eq!(
            shortcut(&character("="), &character("=")),
            Some(Step::In),
            "Ctrl+= zooms in like a browser"
        );
        // Ctrl+Shift+= on a US layout: the key is "=", typed as "+".
        assert_eq!(shortcut(&character("="), &character("+")), Some(Step::In));
        assert_eq!(shortcut(&character("-"), &character("-")), Some(Step::Out));
        assert_eq!(shortcut(&character("-"), &character("_")), Some(Step::Out));
        assert_eq!(
            shortcut(&character("0"), &character("0")),
            Some(Step::Reset)
        );
        assert_eq!(shortcut(&character("a"), &character("a")), None);
        assert_eq!(
            shortcut(&Key::Named(Named::ArrowUp), &Key::Named(Named::ArrowUp)),
            None
        );
    }
}
