//! There are multiple parts of the UI where we want to display a list of sorted items. In some
//! cases, these lists can be extremely large (the log block will store a history of the last 64k
//! events) and have the additional complexity that not all items are the same size (most events
//! are a single line, but some are two or three. This also depends on the list's width!)
//!
//! This is the problem [`LongList`] attempts to solve. It is a view for possibly very long lists
//! of items, that will only process the items that are currently in view, for best efficiency.
//!
//! Items are turned into lines (`Line<'static>`) through an implementation of
//! [`LongListHandler::get_item_lines`] provided by the user of the list. Only the items that are
//! currently in view are turned into lines, and the line instances persist between renders. As
//! the list is scrolled, items that come into view are turned into lines, and the lines of the
//! items that go out of view are droppped.
//!
//! The advantage to this persisting of lines is clearly performance, but the disadvantage is that
//! whenever a change to the list happens, the long list needs to be informed.

use std::{
    cell::RefCell,
    collections::VecDeque,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    text::Line,
    widgets::{Block, BorderType, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use tokio::sync::Notify;

use crate::tui::{
    text_wrapper::StaticString,
    ui_element::{AutosizeUIElement, HandleEventStatus, PassFocusDirection, UIElement},
};

use super::OnEnterResult;

/// The list can be scrolled with the arrow keys, pageup, pagedown, etc. While holding shift, the
/// scroll speed is multiplied by this amount.
const KEY_SHIFT_SCROLL_AMOUNT: usize = 5;

/// An interface for handling a long list.
pub trait LongListHandler {
    /// Gets the lines for the item at a given index.
    fn get_item_lines<F: FnMut(Line<'static>)>(&mut self, index: usize, wrap_width: u16, f: F);

    /// Modifies the line to be displayed as selected.
    fn modify_line_to_selected(&mut self, index: usize, line: &mut Line<'static>, item_line_number: u16);

    /// Modifies a line previously passed to `modify_line_to_selected` back to unselected.
    fn modify_line_to_unselected(&mut self, index: usize, line: &mut Line<'static>, item_line_number: u16);

    /// Called when the ENTER key is pressed while selecting an item.
    fn on_enter(&mut self, index: usize) -> OnEnterResult;
}

/// An efficient display for a list with possibly many items.
pub struct LongList<H: LongListHandler> {
    current_area: Rect,
    controller: Rc<LongListController>,
    pub handler: H,
    tmp_rev_vec: Vec<Line<'static>>,
}

struct LongListControllerInner {
    // The title for the block surrounding the list.
    title: StaticString,

    /// Stores a set of lines and the index of the item to which each one belongs. These are sorted
    /// by index ascending and the indices are contiguous and without gaps.
    lines: VecDeque<(Line<'static>, usize)>,

    /// The amount of items to dispaly. This is the index of the highest item exclusive.
    item_count: usize,

    /// The index of the currently selected item, or None if the list is not focused.
    selected_index: Option<usize>,

    /// Whether to select the last (true) or the first (false) item when the list gains focus.
    select_last_first: bool,

    /// Whether to deselect an item when removed. Otherwise, the next item is selected instead.
    deselect_on_remove: bool,
}

/// Implementation of [`LongListControllerInner::index_of_first`] but the borrow checker won't
/// complain if you are also borrowing another field from the same struct at the same time.
#[inline]
fn calc_index_of_first(item_count: usize) -> Option<usize> {
    match item_count {
        0 => None,
        _ => Some(0),
    }
}

/// Implementation of [`LongListControllerInner::index_of_last`] but the borrow checker won't
/// complain if you are also borrowing another field from the same struct at the same time.
#[inline]
fn calc_index_of_last(item_count: usize) -> Option<usize> {
    match item_count {
        0 => None,
        v => Some(v - 1),
    }
}

/// Implementation of [`LongListControllerInner::selection_start_index`] but the borrow checker
/// won't complain if you are also borrowing another field from the same struct at the same time.
#[inline]
fn calc_index_of_selection_start(select_last_first: bool, item_count: usize) -> Option<usize> {
    match item_count {
        0 => None,
        v if select_last_first => Some(v - 1),
        _ => Some(0),
    }
}

impl LongListControllerInner {
    fn item_count(&self) -> usize {
        self.item_count
    }

    /// Gets the index of the lowest item in the list inclusive, or None if there are no items.
    fn index_of_first(&self) -> Option<usize> {
        calc_index_of_first(self.item_count)
    }

    /// Gets the index of the highest item in the list inclusive, or None if there are no items.
    fn index_of_last(&self) -> Option<usize> {
        calc_index_of_last(self.item_count)
    }

    /// Gets the index of the first item to select according to `self.selection_start_index`.
    fn selection_start_index(&self) -> Option<usize> {
        calc_index_of_selection_start(self.select_last_first, self.item_count)
    }
}

/// Used for controlling a [`LongList`].
pub struct LongListController {
    redraw_notify: Rc<Notify>,
    inner: RefCell<LongListControllerInner>,
}

impl LongListController {
    fn new(redraw_notify: Rc<Notify>, title: StaticString, item_count: usize, select_last_first: bool, deselect_on_remove: bool) -> Self {
        let inner = LongListControllerInner {
            title,
            lines: VecDeque::new(),
            item_count,
            selected_index: None,
            select_last_first,
            deselect_on_remove,
        };

        Self {
            redraw_notify,
            inner: RefCell::new(inner),
        }
    }

    pub fn item_count(&self) -> usize {
        self.inner.borrow().item_count
    }

    /// Resets the list, forgetting any current items and setting a new item count, but without
    /// notifying for redraw.
    pub fn reset_items_no_redraw(&self, item_count: usize, try_keep_selected: bool) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        inner.item_count = item_count;
        inner.lines.clear();

        inner.selected_index = match inner.selected_index {
            Some(idx) if try_keep_selected && idx < item_count => Some(idx),
            _ => None,
        };
    }

    /// Sets the amount of items to display. This equals the highest exclusive index.
    pub fn set_item_count(&self, item_count: usize) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        if inner.item_count == item_count {
            return;
        }

        inner.item_count = item_count;

        // Remove from `lines` all the lines whose index is greater than or equal to item_count.
        while inner.lines.back().is_some_and(|(_, index)| *index >= item_count) {
            inner.lines.pop_back();
        }

        // If there is a selected index and at least one item on the list, clamp the selected index to range.
        inner.selected_index = inner.selected_index.zip(inner.index_of_last()).map(|(si, il)| si.min(il));

        self.redraw_notify.notify_one();
    }

    /// Indicates to the list that an item was removed. All items after it are decremented in index
    /// and the item count is decremented too.
    ///
    /// The handling of removing the currently selected item is done according to the
    /// `deselect_on_remove` boolean value, by either deselecting or selecting the next item.
    pub fn on_item_removed(&self, index: usize) {
        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        inner.item_count = inner.item_count.saturating_sub(1);

        // Find the index in `inner.lines` where the lines with index `index` are (if present).
        let mut iter = inner.lines.iter().enumerate().skip_while(|(_, (_, idx))| *idx < index);
        let index_in_lines = iter.next().filter(|(_, (_, idx))| *idx == index).map(|(i, _)| i);

        // If present, count how many lines there are with said index and remove them in bulk.
        let decrese_indexes_after = if let Some(index_in_lines) = index_in_lines {
            let count = inner.lines.range(index_in_lines..).take_while(|(_, idx)| *idx == index).count();
            inner.lines.drain(index_in_lines..(index_in_lines + count));
            index_in_lines
        } else if inner.lines.front().map(|(_, idx)| *idx).is_some_and(|idx| idx > index) {
            0
        } else {
            inner.lines.len()
        };

        // Decrement the index of lines whose index is greater than the removed (if needed).
        for tuple in inner.lines.range_mut(decrese_indexes_after..) {
            tuple.1 -= 1;
        }

        // If the selected index was at or after the removed item, deselect or decrement it.
        if let Some(selected_index) = &mut inner.selected_index {
            if (inner.deselect_on_remove && *selected_index == index) || inner.item_count == 0 {
                inner.selected_index = None;
            } else if *selected_index >= index {
                *selected_index = selected_index.saturating_sub(1);
            }
        }

        self.redraw_notify.notify_one();
    }
}

impl<H: LongListHandler> LongList<H> {
    pub fn new(
        redraw_notify: Rc<Notify>,
        title: StaticString,
        item_count: usize,
        select_last_first: bool,
        deselect_on_remove: bool,
        handler: H,
    ) -> Self {
        let controller = Rc::new(LongListController::new(
            redraw_notify,
            title,
            item_count,
            select_last_first,
            deselect_on_remove,
        ));

        Self {
            current_area: Rect::default(),
            controller,
            handler,
            tmp_rev_vec: Vec::new(),
        }
    }

    fn get_focus_position(&self) -> (u16, u16) {
        (self.current_area.x, self.current_area.y + self.current_area.height / 2)
    }

    fn handle_mouse_event(&mut self, mouse_event: &MouseEvent, is_focused: bool) -> HandleEventStatus {
        if !is_focused {
            return HandleEventStatus::Unhandled;
        }

        let is_up = match mouse_event.kind {
            MouseEventKind::ScrollUp => true,
            MouseEventKind::ScrollDown => false,
            _ => return HandleEventStatus::Unhandled,
        };

        let mut inner_guard = self.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        let previous_selected_index = inner.selected_index;

        if let Some(selected_index) = &mut inner.selected_index {
            let scroll_amount = match mouse_event.modifiers {
                m if m.contains(KeyModifiers::SHIFT) => KEY_SHIFT_SCROLL_AMOUNT,
                _ => 1,
            };

            *selected_index = match is_up {
                true => selected_index.saturating_sub(scroll_amount),
                false => (*selected_index + scroll_amount).min(inner.item_count.saturating_sub(1)),
            };
        } else if let Some(index_to_select) = inner.selection_start_index() {
            inner.selected_index = Some(index_to_select);
        }

        if inner.selected_index != previous_selected_index {
            self.redraw_notify.notify_one();
        }

        HandleEventStatus::Handled
    }

    fn handle_key_event(&mut self, key_event: &KeyEvent, is_focused: bool) -> HandleEventStatus {
        if !is_focused || key_event.kind == KeyEventKind::Release {
            return HandleEventStatus::Unhandled;
        }

        if key_event.modifiers.contains(KeyModifiers::CONTROL) {
            match key_event.code {
                KeyCode::Up => return HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
                KeyCode::Down => return HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
                // Note: Left and Right keys already pass focus on the match below this one.
                _ => {}
            }
        }

        let mut inner_guard = self.controller.inner.borrow_mut();
        let inner = inner_guard.deref_mut();
        let previous_selected_index = inner.selected_index;

        let return_value = match key_event.code {
            KeyCode::Up => {
                let pass_focus = match &mut inner.selected_index {
                    None => {
                        inner.selected_index = inner.selection_start_index();
                        inner.selected_index.is_none()
                    }
                    Some(selected_index) => {
                        if let Some(index_of_first) = calc_index_of_first(inner.item_count) {
                            if *selected_index == index_of_first {
                                true
                            } else {
                                let scroll_amount = match key_event.modifiers {
                                    m if m.contains(KeyModifiers::SHIFT) => KEY_SHIFT_SCROLL_AMOUNT,
                                    _ => 1,
                                };

                                *selected_index = selected_index.saturating_sub(scroll_amount).max(index_of_first);
                                false
                            }
                        } else {
                            true
                        }
                    }
                };

                match pass_focus {
                    true => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Up),
                    false => HandleEventStatus::Handled,
                }
            }
            KeyCode::Down => {
                let pass_focus = match &mut inner.selected_index {
                    None => {
                        inner.selected_index = inner.selection_start_index();
                        inner.selected_index.is_none()
                    }
                    Some(selected_index) => {
                        if let Some(index_of_last) = calc_index_of_last(inner.item_count) {
                            if *selected_index == index_of_last {
                                true
                            } else {
                                let scroll_amount = match key_event.modifiers {
                                    m if m.contains(KeyModifiers::SHIFT) => KEY_SHIFT_SCROLL_AMOUNT,
                                    _ => 1,
                                };

                                *selected_index = selected_index.saturating_add(scroll_amount).min(index_of_last);
                                false
                            }
                        } else {
                            true
                        }
                    }
                };

                match pass_focus {
                    true => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Down),
                    false => HandleEventStatus::Handled,
                }
            }
            KeyCode::PageUp => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                match &mut inner.selected_index {
                    None => {
                        inner.selected_index = inner.selection_start_index();
                    }
                    Some(selected_index) => {
                        let scroll_amount = match key_event.modifiers {
                            m if m.contains(KeyModifiers::SHIFT) => KEY_SHIFT_SCROLL_AMOUNT,
                            _ => 1,
                        } * (self.current_area.height.saturating_sub(2) as usize);

                        *selected_index = selected_index.saturating_sub(scroll_amount);
                    }
                }

                HandleEventStatus::Handled
            }
            KeyCode::PageDown => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                match &mut inner.selected_index {
                    None => {
                        inner.selected_index = inner.selection_start_index();
                    }
                    Some(selected_index) => {
                        if let Some(index_of_last) = calc_index_of_last(inner.item_count) {
                            let scroll_amount = match key_event.modifiers {
                                m if m.contains(KeyModifiers::SHIFT) => KEY_SHIFT_SCROLL_AMOUNT,
                                _ => 1,
                            } * (self.current_area.height.saturating_sub(2) as usize);

                            *selected_index = selected_index.saturating_add(scroll_amount).min(index_of_last);
                        }
                    }
                }

                HandleEventStatus::Handled
            }
            KeyCode::Home => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                inner.selected_index = inner.index_of_first();
                HandleEventStatus::Handled
            }
            KeyCode::End => {
                if !is_focused {
                    return HandleEventStatus::Unhandled;
                }

                inner.selected_index = inner.index_of_last();
                HandleEventStatus::Handled
            }
            KeyCode::Left => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Left),
            KeyCode::Right => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Right),
            KeyCode::Tab => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Forward),
            KeyCode::Esc => HandleEventStatus::PassFocus(self.get_focus_position(), PassFocusDirection::Away),
            KeyCode::Enter => {
                if let Some(selected_index) = inner.selected_index {
                    match self.handler.on_enter(selected_index) {
                        OnEnterResult::Handled => HandleEventStatus::Handled,
                        OnEnterResult::Unhandled => HandleEventStatus::Unhandled,
                        OnEnterResult::PassFocus(direction) => HandleEventStatus::PassFocus(self.get_focus_position(), direction),
                    }
                } else {
                    HandleEventStatus::Unhandled
                }
            }
            _ => HandleEventStatus::Unhandled,
        };

        if inner.selected_index != previous_selected_index {
            self.redraw_notify.notify_one();
        }

        return_value
    }

    /// Ensures `inner.lines` contains the lines for the item at the index `center_item_index`, as
    /// well as at least `area.height` lines around it as to display them. The lines are then
    /// passed to the `draw_line_function` closure ordered by ascending index.
    ///
    /// Lines are stored in `inner.lines`, and when missing they are taken by item index from
    /// `handler.get_item_lines`. If a line is currently selected, it is first passed to
    /// `handler.modify_line_to_selected` before calling the draw closure, and afterwards again
    /// passed to `modify_line_to_unselected`.
    ///
    /// The item index `center_item_index` is assumed to be form an existing index, unless
    /// `inner.item_count` is zero, in which case this function does nothing.
    ///
    /// ### Small Note
    /// This function was originally in `log_block.rs` and intended to work only with log events.
    /// This list implementation was generalized for use in other parts of the UI. Even though the
    /// logic within was simplified, the warning below remains as a hearitage of its history.
    ///
    /// ## Dear reader;
    /// This function took a whole day to write. From morning to shut-eye. Whole day of doing
    /// absolutely nothing else but sitting on the computer. Staring at the screen. Thinking about
    /// how to tacke the problem. Rewriting it because "nah that's not elegant". Typing and typing.
    /// And debugging to find that I was simply missing a "-1" somewhere.
    ///
    /// And that's not because it's particularly long, or inefficient, or hacky, or convoluted. On
    /// thecontrary; I think it's elegant, performant, efficient, and the exact solution to the
    /// problem at hand. However, it's also quite complex and very much non-trivial.
    ///
    /// Even though I haven't commented most of this codebase, if I didn't comment this function at
    /// all then soon enough it would only be understood by the One True God, השם, HaShem.
    ///
    /// As I write this, at 23:57, I mark on this very file a grave cautionary warning; _don't read
    /// this too hard if you value your mental sanity_. My comments will well serve adventurous
    /// explorers, but not all may make it through the journey.
    fn process_lines<F: FnMut(&mut Line<'static>)>(&mut self, center_item_index: usize, area: Rect, mut draw_line_function: F) {
        let mut inner_guard = self.controller.inner.borrow_mut();
        let inner = inner_guard.deref_mut();

        if inner.item_count == 0 || area.is_empty() {
            return;
        }

        // We start by looking for where in `inner.lines` the item with center_item_index is (or is not).
        // This isn't an expensive operation; `inner.lines` only carries the lines needed to display the
        // items visible in the viewport for a frame, which is a small amount.
        let maybe_center_start_line_i = inner.lines.binary_search_by_key(&center_item_index, |(_, id)| *id);

        // Calculate the item's index in `inner.lines` and the amount of lines it has.
        let (center_start_line_i, center_line_count) = match maybe_center_start_line_i {
            Ok(mut index) => {
                // The item is already in `inner.lines`, but the entry that the binary search found
                // might not be the first line (an item might have multiple lines), so let's backtrack
                // until we found the first line of the item.
                while index != 0 && inner.lines[index - 1].1 == center_item_index {
                    index -= 1;
                }

                // Now let's count how many lines the item has. Items are not intended to have many lines,
                // so no need for complex searching, a simple linear walk will do.
                let mut line_count = 1;
                while let Some((_, id)) = inner.lines.get(index + line_count) {
                    if *id != center_item_index {
                        break;
                    }

                    line_count += 1;
                }

                (index, line_count)
            }
            Err(_) => {
                // The item is not in `inner.lines`. We clear the lines deque and add the item alone.
                inner.lines.clear();
                self.handler.get_item_lines(center_item_index, area.width, |line| {
                    inner.lines.push_back((line, center_item_index));
                });

                (0, inner.lines.len())
            }
        };

        let additional_line_count = (area.height as usize).saturating_sub(center_line_count) as i32;

        let mut remaining_lower_lines_count = additional_line_count / 2;
        let mut lower_items_current_index = center_item_index;
        let mut lines_lower_item_current_i = center_start_line_i;

        let mut remaining_higher_lines_count = (additional_line_count + 1) / 2;
        let mut higher_items_next_index = center_item_index + 1;
        let mut lines_higher_item_next_i = center_start_line_i + 1;
        while lines_higher_item_next_i != inner.lines.len() && inner.lines[lines_higher_item_next_i].1 == center_item_index {
            lines_higher_item_next_i += 1;
        }

        // Turn items into lines and add them to `inner.lines` progressively, side by side, until there
        // are enough lines on each side or there aren't any more items. Items might already have been
        // turned into lines before, and thus might already be in `inner.lines`, in which case they'll
        // be reused.
        while (remaining_lower_lines_count > 0 || remaining_higher_lines_count > 0)
            && (lower_items_current_index > 0 || higher_items_next_index < inner.item_count())
        {
            // Check on which side we need more lines, we'll add lines on the side that needs more.
            if remaining_higher_lines_count < remaining_lower_lines_count {
                // We're adding lines at the start (lower indices).
                if lower_items_current_index == 0 {
                    // There are no more lower items! Pass over the remaining to the higher side, so we add
                    // more lines with higher items to compensate and get the same total amount of lines.
                    remaining_higher_lines_count += remaining_lower_lines_count;
                    remaining_lower_lines_count = 0;
                } else {
                    // There is another lower item. If it's already parsed into lines into `inner.lines`, we
                    // count those lines and skip over them. Otherwise, we convert the item into lines and
                    // add them into `inner.lines`.
                    lower_items_current_index -= 1;
                    if lines_lower_item_current_i == 0 {
                        // Parse the next lower item into lines and push them to the front of `inner.lines`.
                        self.tmp_rev_vec.clear();
                        let item_index = lower_items_current_index;
                        self.handler.get_item_lines(item_index, area.width, |line| {
                            self.tmp_rev_vec.push(line);
                        });

                        for line in self.tmp_rev_vec.drain(..).rev() {
                            inner.lines.push_front((line, item_index));
                            remaining_lower_lines_count -= 1;
                            lines_higher_item_next_i += 1;
                            // ^^ Pushing elements to the lower indices also pushes forward the higher indices!
                        }
                    } else {
                        // Count and skip over the pre-existing parsed lines of this item.
                        lines_lower_item_current_i -= 1;
                        remaining_lower_lines_count -= 1;
                        let skipping_item_index = inner.lines[lines_lower_item_current_i].1;
                        while lines_lower_item_current_i != 0 && inner.lines[lines_lower_item_current_i - 1].1 == skipping_item_index {
                            lines_lower_item_current_i -= 1;
                            remaining_lower_lines_count -= 1;
                        }
                    }
                }
            } else {
                // We're adding lines at the end (higher indices).
                if higher_items_next_index == inner.item_count() {
                    // There are no more higher items! Pass over the remaining to the lower side, so we add
                    // more lines with lower items to compensate and get the same total amount of lines.
                    remaining_lower_lines_count += remaining_higher_lines_count;
                    remaining_higher_lines_count = 0;
                } else {
                    // There is another higher item. If it's already parsed into lines into `inner.lines`, we
                    // count those lines and skip over them. Otherwise, we convert the item into lines and
                    // add them into `inner.lines`.
                    if lines_higher_item_next_i == inner.lines.len() {
                        // Parse the next higher item into lines and push them to the back of `inner.lines`.
                        let item_index = higher_items_next_index;
                        self.handler.get_item_lines(item_index, area.width, |line| {
                            inner.lines.push_back((line, item_index));
                            remaining_higher_lines_count -= 1;
                        });

                        lines_higher_item_next_i = inner.lines.len();
                    } else {
                        // Count and skip over the pre-existing parsed lines of this item.
                        let skipping_item_index = inner.lines[lines_higher_item_next_i].1;
                        lines_higher_item_next_i += 1;
                        remaining_higher_lines_count -= 1;
                        while lines_higher_item_next_i != inner.lines.len()
                            && inner.lines[lines_higher_item_next_i].1 == skipping_item_index
                        {
                            lines_higher_item_next_i += 1;
                            remaining_higher_lines_count -= 1;
                        }
                    }
                    higher_items_next_index += 1;
                }
            }
        }

        // Remove unneeded lines from the deque. We remove the back (higher indices) before the
        // front (lower indices) because removing elements from the lower indices will alter the
        // indices of the higher elements. (If you don't like the lower indices being the front and
        // the higher indices being the back, yeah... I get you. But that's VecDeque terminology).
        for _ in lines_higher_item_next_i..inner.lines.len() {
            inner.lines.pop_back();
        }
        for _ in 0..lines_lower_item_current_i {
            inner.lines.pop_front();
            lines_higher_item_next_i -= 1;
        }

        // Get the lines to draw as an iterator. The iterator must only cover the parts of the
        // lines deque that should be rendered. Fortunately, there's an easy way we can find that
        // using leftover variables from the previous calculation. The "remaining" counters might
        // be negative, if more lines were found/added than exactly necessary (what can happen if,
        // for example, we need one more line but the next item uses two). Thankfully this is just
        // what we need! If they are negative, that's how many leftover lines we have on each side.
        let mut iter = inner.lines.iter_mut();
        for _ in 0..(-remaining_lower_lines_count) {
            iter.next();
        }
        for _ in 0..(-remaining_higher_lines_count) {
            iter.next_back();
        }

        // Still, in some edge cases (an item with more lines than needed_line_count) we might end
        // up with more lines than we should. We can simply limit the iterator to fix this.
        let iter = iter.take(area.height as usize);

        // Iterate through the lines and draw them. For each line we also need to calculate its
        // line number within the item (if an item has three lines, they are numbered 0, 1, and 2),
        // as this information is required by the handler for the `modify_line_to_selected` logic.
        let mut previous_item_index = None;
        let mut item_line_number = (-remaining_lower_lines_count).max(0) as u16;
        for (line, item_index) in iter {
            if previous_item_index.is_some_and(|prev_idx| prev_idx != *item_index) {
                item_line_number = 0;
            }

            let is_selected = inner.selected_index == Some(*item_index);
            if is_selected {
                self.handler.modify_line_to_selected(*item_index, line, item_line_number);
            }

            draw_line_function(line);

            if is_selected {
                self.handler.modify_line_to_unselected(*item_index, line, item_line_number);
            }

            item_line_number += 1;
            previous_item_index = Some(*item_index);
        }
    }
}

impl<H: LongListHandler> Deref for LongList<H> {
    type Target = Rc<LongListController>;

    /// Gets a reference to this list's controller.
    ///
    /// ## Note
    /// `LongList` does not implement methods like `set_item_count` or `on_item_removed`, but
    /// rather these are implemented in the controller. The implementation of [`Deref`] for
    /// `LongList` makes it possible to call `long_list.set_item_count(...)` without having to
    /// explicitly grab the controller from the list.
    fn deref(&self) -> &Self::Target {
        &self.controller
    }
}

impl<H: LongListHandler> UIElement for LongList<H> {
    fn resize(&mut self, area: Rect) {
        let previous_area = self.current_area;
        self.current_area = area;

        if previous_area.width != area.width || previous_area.height != area.height {
            self.inner.borrow_mut().lines.clear();
        }
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let inner_guard = self.inner.borrow();
        let inner = inner_guard.deref();

        let block = Block::new()
            .border_type(BorderType::Plain)
            .borders(Borders::ALL)
            .title(inner.title.deref());

        let list_area = block.inner(area);
        frame.render_widget(block, area);

        let selected_index = inner.selected_index;
        let scrollbar_content_length = inner.item_count.saturating_sub(list_area.height as usize);
        let scrollbar_position = match selected_index {
            Some(idx) => scrollbar_content_length * idx / (inner.item_count - 1).max(1),
            None => scrollbar_content_length,
        };

        let mut scrollbar_state = ScrollbarState::new(scrollbar_content_length)
            .viewport_content_length(list_area.height as usize)
            .position(scrollbar_position);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);

        let center_item_index = selected_index.unwrap_or_else(|| inner.selection_start_index().unwrap_or(0));
        drop(inner_guard);

        let buf = frame.buffer_mut();
        let mut y = list_area.top();
        self.process_lines(center_item_index, list_area, move |line| {
            buf.set_line(list_area.left(), y, line, list_area.width);
            y += 1;
        });
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        match event {
            event::Event::Mouse(mouse_event) => self.handle_mouse_event(mouse_event, is_focused),
            event::Event::Key(key_event) => self.handle_key_event(key_event, is_focused),
            _ => HandleEventStatus::Unhandled,
        }
    }

    fn receive_focus(&mut self, _focus_position: (u16, u16)) -> bool {
        let mut inner = self.inner.borrow_mut();
        if let Some(index_to_select) = inner.selection_start_index() {
            inner.selected_index = Some(index_to_select);
            self.redraw_notify.notify_one();
            true
        } else {
            false
        }
    }

    fn focus_lost(&mut self) {
        let mut inner = self.inner.borrow_mut();
        if inner.selected_index.is_some() {
            inner.selected_index = None;
            self.redraw_notify.notify_one();
        }
    }
}

impl<H: LongListHandler> AutosizeUIElement for LongList<H> {
    fn begin_resize(&mut self, width: u16, height: u16) -> (u16, u16) {
        let desired_height = self.controller.item_count().saturating_add(2).min(u16::MAX as usize) as u16;
        (width, desired_height.min(height))
    }
}
