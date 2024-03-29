use crossterm::event;
use ratatui::{layout::Rect, Frame};

/// Represents a visual element on the UI.
pub trait UIElement {
    /// Indicates the area on the screen this [`UIElement`] will draw to. Future calls to `render`
    /// will pass the same area as parameter.
    fn resize(&mut self, area: Rect);

    /// A counterpart to [`Widget`][ratatui::widgets::Widget], but with `&mut self` instead of
    /// `self`.
    fn render(&mut self, area: Rect, frame: &mut Frame);

    /// Handles an input event, such as keyboard and/or mouse. Returns a [`HandleEventStatus`],
    /// which indicates whether the event was handled, unhandled, or requests to pass the focus
    /// to another element.
    ///
    /// When an event occurs, this function is called for UI elements in order of importance.
    /// When an event is handled (by returning either `Handled` or `PassFocus`), the event not
    /// passed further. Implementations should never return `PassFocus` unless they are currently
    /// in focus.
    ///
    /// Note that elements lose focus when they return `PassFocus`, but it is possible for focus
    /// to be taken away from an element without `handle_event` being called.
    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus;

    /// Offers the focus to this element. Focused elements receive input events first, before any
    /// other elements, and can thus prevent all other elements from receiving events if they want.
    ///
    /// The "focus position" is an indicator of where the previous focused element was, which is
    /// intended to be used for determining which is the new focused element.
    ///
    /// Returns true if the focus is accepted, false otherwise. If false, focus may be offered to
    /// another element.
    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool;

    /// Called when this element loses focus.
    ///
    /// This is called after this element returns [`HandleEventStatus::PassFocus`] from a call to
    /// [`handle_event`][UIElement::handle_event], or if focus is being taken away from this
    /// element.
    fn focus_lost(&mut self);
}

/// The status of handling an UI element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleEventStatus {
    /// The event is handled and should not be passed to any other UI elements.
    Handled,

    /// The event remains unhandled and may be passed onto other UI elements.
    Unhandled,

    /// The event is handled and this UI element requests that its focus be taken away and offered
    /// to another UI element. This includes a location of where the focus was, which is intended
    /// for better choosing the new focus element, as well as a direction (if, for example, an
    /// arrow key was pressed to trigger this).
    PassFocus((u16, u16), PassFocusDirection),
}

impl HandleEventStatus {
    pub fn or_else<F: FnOnce() -> Self>(self, f: F) -> Self {
        match self {
            HandleEventStatus::Unhandled => f(),
            other => other,
        }
    }
}

/// Directions for passing a focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassFocusDirection {
    /// Do not offer the focus to any element (unfocus).
    Away,

    /// No specific direction for passing focus (useful for the TAB key).
    Forward,

    /// Pass focus upwards.
    Up,

    /// Pass focus downwards.
    Down,

    /// Pass focus leftwards.
    Left,

    /// Pass focus rightwards.
    Right,
}

/// Represents an [`UIElement`] that can request a size during resize operations.
pub trait AutosizeUIElement: UIElement {
    /// Called before [`UIElement::resize`] with the maximum available size, and returns this
    /// element's desired size. After this call, `resize` will be called with the final size.
    ///
    /// Elements should only ask for exactly as much space as they need and no more. Asking for
    /// more space might mean other elements are not given any space at all.
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16);
}
