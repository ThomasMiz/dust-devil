use std::{cell::RefCell, rc::Rc};

use crossterm::event;
use dust_devil_core::{logging, sandstorm::Metrics};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    Frame,
};
use tokio::sync::Notify;

use crate::tui::ui_element::AutosizeUIElement;

use self::{log_block::LogBlock, metrics_display::MetricsDisplay, usage_graph::UsageGraph};

use super::{
    elements::{
        arrow_selector::{ArrowSelector, ArrowSelectorHandler},
        centered_button::{ButtonHandler, CenteredButton},
        horizontal_split::HorizontalSplit,
        text::TextLine,
        OnEnterResult,
    },
    ui_element::{HandleEventStatus, PassFocusDirection, UIElement},
};

mod colored_logs;
mod log_block;
mod metrics_display;
mod usage_graph;
mod usage_tracker;

const CLIENT_ACTIVITY_LABEL: &str = "Client Activity";
const EXPAND_GRAPH_LABEL: &str = "[expand graph (g)]";
const EXPAND_GRAPH_SHORTCUT: Option<char> = Some('g');
const RETURN_TO_MAIN_VIEW_LABEL: &str = "[return to main view (q)]";
const RETURN_TO_MAIN_VIEW_SHORTCUT: Option<char> = Some('q');
const GRAPH_PRECISION_LABEL: &str = "Graph precision:";
const PRECISION_SELECTOR_AFTER_TEXT: &str = "[change (1/2/3...9)]";

const SELECTED_BACKGROUND_COLOR: Color = Color::DarkGray;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum GraphPrecisionOption {
    #[default]
    OneSecond = 1,
    TwoSeconds = 2,
    FiveSeconds = 5,
    TenSeconds = 10,
    ThirtySeconds = 30,
    OneMinute = 60,
    TwoMinutes = 120,
    FiveMinutes = 300,
    TenMinutes = 600,
}

const ALL_GRAPH_PRECISION_OPTIONS: &[GraphPrecisionOption] = &[
    GraphPrecisionOption::OneSecond,
    GraphPrecisionOption::TwoSeconds,
    GraphPrecisionOption::FiveSeconds,
    GraphPrecisionOption::TenSeconds,
    GraphPrecisionOption::ThirtySeconds,
    GraphPrecisionOption::OneMinute,
    GraphPrecisionOption::TwoMinutes,
    GraphPrecisionOption::FiveMinutes,
    GraphPrecisionOption::TenMinutes,
];

impl GraphPrecisionOption {
    fn to_str(self) -> &'static str {
        match self {
            Self::OneSecond => "1s",
            Self::TwoSeconds => "2s",
            Self::FiveSeconds => "5s",
            Self::TenSeconds => "10s",
            Self::ThirtySeconds => "30s",
            Self::OneMinute => "1m",
            Self::TwoMinutes => "2m",
            Self::FiveMinutes => "5m",
            Self::TenMinutes => "10m",
        }
    }

    fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::OneSecond),
            1 => Some(Self::TwoSeconds),
            2 => Some(Self::FiveSeconds),
            3 => Some(Self::TenSeconds),
            4 => Some(Self::ThirtySeconds),
            5 => Some(Self::OneMinute),
            6 => Some(Self::TwoMinutes),
            7 => Some(Self::FiveMinutes),
            8 => Some(Self::TenMinutes),
            _ => None,
        }
    }

    /// Returns (unit_size_seconds, labels_on_multiples_of: u32, markers_on_multiples_of, print_seconds)
    fn get_values(self) -> (u32, u32, u32, bool) {
        match self {
            Self::OneSecond => (1, 15, 5, true),
            Self::TwoSeconds => (2, 30, 10, true),
            Self::FiveSeconds => (5, 60, 20, false),
            Self::TenSeconds => (10, 120, 30, false),
            Self::ThirtySeconds => (30, 240, 120, false),
            Self::OneMinute => (60, 600, 150, false),
            Self::TwoMinutes => (120, 1200, 300, false),
            Self::FiveMinutes => (300, 3600, 1200, false),
            Self::TenMinutes => (600, 7200, 1800, false),
        }
    }
}

pub struct MainView {
    log_block: LogBlock,
    log_block_area: Rect,
    usage_graph: UsageGraph,
    usage_graph_area: Rect,
    metrics_display: MetricsDisplay,
    metrics_display_area: Rect,
    client_activity_line: HorizontalSplit<TextLine, CenteredButton<ExpandButtonHandler>>,
    client_activity_line_area: Rect,
    graph_precision_line: HorizontalSplit<TextLine, ArrowSelector<PrecisionSelectorHandler>>,
    graph_precision_line_area: Rect,
    controller: Rc<Controller>,
    layout_mode: LayoutMode,
    is_focused: bool,
    focused_element: FocusedElement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Full,
    LogsOnly,
    GraphExpanded,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedElement {
    LogBlock,
    ExpandButton,
    PrecisionSelector,
}

impl FocusedElement {
    fn is_visible_in(self, layout_mode: LayoutMode) -> bool {
        match layout_mode {
            LayoutMode::Full => match self {
                Self::LogBlock => true,
                Self::ExpandButton => true,
                Self::PrecisionSelector => true,
            },
            LayoutMode::LogsOnly => match self {
                Self::LogBlock => true,
                Self::ExpandButton => true,
                Self::PrecisionSelector => false,
            },
            LayoutMode::GraphExpanded => match self {
                Self::LogBlock => false,
                Self::ExpandButton => true,
                Self::PrecisionSelector => true,
            },
        }
    }

    fn up(self, layout_mode: LayoutMode) -> Option<Self> {
        match layout_mode {
            LayoutMode::Full => match self {
                Self::LogBlock | Self::ExpandButton => None,
                Self::PrecisionSelector => Some(Self::ExpandButton),
            },
            LayoutMode::LogsOnly => match self {
                Self::LogBlock | Self::PrecisionSelector => Some(Self::ExpandButton),
                Self::ExpandButton => None,
            },
            LayoutMode::GraphExpanded => match self {
                Self::LogBlock | Self::ExpandButton | Self::PrecisionSelector => None,
            },
        }
    }

    fn down(self, layout_mode: LayoutMode) -> Option<Self> {
        match layout_mode {
            LayoutMode::Full => match self {
                Self::LogBlock | Self::PrecisionSelector => None,
                Self::ExpandButton => Some(Self::PrecisionSelector),
            },
            LayoutMode::LogsOnly => match self {
                Self::LogBlock => None,
                Self::ExpandButton | Self::PrecisionSelector => Some(Self::LogBlock),
            },
            LayoutMode::GraphExpanded => match self {
                Self::LogBlock | Self::ExpandButton | Self::PrecisionSelector => None,
            },
        }
    }

    fn left(self, layout_mode: LayoutMode) -> Option<Self> {
        match layout_mode {
            LayoutMode::Full => match self {
                Self::LogBlock => None,
                Self::ExpandButton | Self::PrecisionSelector => Some(Self::LogBlock),
            },
            LayoutMode::LogsOnly => match self {
                Self::LogBlock | Self::ExpandButton | Self::PrecisionSelector => None,
            },
            LayoutMode::GraphExpanded => match self {
                Self::LogBlock | Self::ExpandButton => None,
                Self::PrecisionSelector => Some(Self::ExpandButton),
            },
        }
    }

    fn right(self, layout_mode: LayoutMode) -> Option<Self> {
        match layout_mode {
            LayoutMode::Full => match self {
                Self::LogBlock => Some(Self::PrecisionSelector),
                Self::ExpandButton | Self::PrecisionSelector => None,
            },
            LayoutMode::LogsOnly => match self {
                Self::LogBlock | Self::ExpandButton | Self::PrecisionSelector => None,
            },
            LayoutMode::GraphExpanded => match self {
                Self::LogBlock | Self::PrecisionSelector => None,
                Self::ExpandButton => Some(Self::PrecisionSelector),
            },
        }
    }

    fn forward(self, layout_mode: LayoutMode) -> Option<Self> {
        match layout_mode {
            LayoutMode::Full => match self {
                Self::LogBlock => Some(Self::ExpandButton),
                Self::ExpandButton => Some(Self::PrecisionSelector),
                Self::PrecisionSelector => None,
            },
            LayoutMode::LogsOnly => match self {
                Self::LogBlock => None,
                Self::ExpandButton | Self::PrecisionSelector => Some(Self::LogBlock),
            },
            LayoutMode::GraphExpanded => match self {
                Self::LogBlock | Self::PrecisionSelector => None,
                Self::ExpandButton => Some(Self::PrecisionSelector),
            },
        }
    }
}

struct ControllerInner {
    expanded: bool,
}

struct Controller {
    redraw_notify: Rc<Notify>,
    inner: RefCell<ControllerInner>,
}

impl Controller {
    fn new(redraw_notify: Rc<Notify>) -> Self {
        let inner = RefCell::new(ControllerInner { expanded: false });

        Self { redraw_notify, inner }
    }

    fn toggle_expanded(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.expanded = !inner.expanded;
        self.redraw_notify.notify_one();
    }
}

struct ExpandButtonHandler {
    controller: Rc<Controller>,
}

impl ExpandButtonHandler {
    fn new(controller: Rc<Controller>) -> Self {
        Self { controller }
    }
}

impl ButtonHandler for ExpandButtonHandler {
    fn on_pressed(&mut self) -> OnEnterResult {
        self.controller.toggle_expanded();
        OnEnterResult::Handled
    }
}

struct PrecisionSelectorHandler {
    redraw_notify: Rc<Notify>,
    usage_graph_controller: Rc<usage_graph::Controller>,
}

impl PrecisionSelectorHandler {
    fn new(redraw_notify: Rc<Notify>, usage_graph_controller: Rc<usage_graph::Controller>) -> Self {
        Self {
            redraw_notify,
            usage_graph_controller,
        }
    }
}

impl ArrowSelectorHandler for PrecisionSelectorHandler {
    fn selection_changed(&mut self, selected_index: usize) {
        let precision_option = GraphPrecisionOption::from_index(selected_index as u8).unwrap();
        self.usage_graph_controller.set_precision(precision_option);
        self.redraw_notify.notify_one();
    }
}

impl MainView {
    pub fn new(redraw_notify: Rc<Notify>, metrics: Metrics) -> Self {
        let controller = Rc::new(Controller::new(Rc::clone(&redraw_notify)));

        let log_block = LogBlock::new(Rc::clone(&redraw_notify));
        let usage_graph = UsageGraph::new(Rc::clone(&redraw_notify));
        let metrics_display = MetricsDisplay::new(Rc::clone(&redraw_notify), metrics);

        let text_style = Style::new();
        let selected_text_style = Style::new().bg(SELECTED_BACKGROUND_COLOR);

        let client_activity_label = TextLine::new(CLIENT_ACTIVITY_LABEL.into(), text_style, Alignment::Left);
        let expand_graph_button = CenteredButton::new(
            Rc::clone(&redraw_notify),
            EXPAND_GRAPH_LABEL.into(),
            text_style,
            selected_text_style,
            EXPAND_GRAPH_SHORTCUT,
            ExpandButtonHandler::new(Rc::clone(&controller)),
        );

        let client_activity_line = HorizontalSplit::new(client_activity_label, expand_graph_button, CLIENT_ACTIVITY_LABEL.len() as u16, 1);

        let graph_precision_label = TextLine::new(GRAPH_PRECISION_LABEL.into(), text_style, Alignment::Left);
        let options_iter = ALL_GRAPH_PRECISION_OPTIONS.iter();
        let mut options_keys = '1'..='9';
        let options_iter = options_iter.map(|x| (x.to_str().into(), options_keys.next()));

        let graph_precision_selector = ArrowSelector::new(
            Rc::clone(&redraw_notify),
            options_iter.collect(),
            0,
            text_style,
            selected_text_style,
            selected_text_style,
            selected_text_style,
            PRECISION_SELECTOR_AFTER_TEXT.into(),
            false,
            PrecisionSelectorHandler::new(redraw_notify, usage_graph.controller()),
        );

        let graph_precision_line = HorizontalSplit::new(
            graph_precision_label,
            graph_precision_selector,
            GRAPH_PRECISION_LABEL.len() as u16,
            1,
        );

        Self {
            log_block,
            log_block_area: Rect::default(),
            usage_graph,
            usage_graph_area: Rect::default(),
            metrics_display,
            metrics_display_area: Rect::default(),
            client_activity_line,
            client_activity_line_area: Rect::default(),
            graph_precision_line,
            graph_precision_line_area: Rect::default(),
            controller,
            layout_mode: LayoutMode::Full,
            is_focused: false,
            focused_element: FocusedElement::LogBlock,
        }
    }

    pub fn new_stream_event(&mut self, event: logging::Event) {
        match &event.data {
            logging::EventData::ClientBytesSent(_, count) => {
                self.metrics_display.on_client_bytes_sent(*count);
                self.usage_graph.record_usage(event.timestamp, *count, 0);
            }
            logging::EventData::ClientBytesReceived(_, count) => {
                self.metrics_display.on_client_bytes_received(*count);
                self.usage_graph.record_usage(event.timestamp, 0, *count);
            }
            logging::EventData::NewClientConnectionAccepted(_, _) => {
                self.metrics_display.on_new_client_connection_accepted();
            }
            logging::EventData::ClientConnectionFinished(_, _, _, _) => {
                self.metrics_display.on_client_connection_finished();
            }
            logging::EventData::NewSandstormConnectionAccepted(_, _) => {
                self.metrics_display.on_new_sandstorm_collection_accepted();
            }
            logging::EventData::SandstormConnectionFinished(_, _) => {
                self.metrics_display.on_sandstorm_collection_finished();
            }
            _ => {}
        }

        self.log_block.new_stream_event(event);
    }

    /// Lays out the elements and sets their areas to their respective `_area` variables. Empty
    /// rectangles indicate the element is not to be rendered.
    fn distribute_areas(&mut self, area: Rect) {
        self.log_block_area = Rect::default();
        self.usage_graph_area = Rect::default();
        self.metrics_display_area = Rect::default();
        self.client_activity_line_area = Rect::default();
        self.graph_precision_line_area = Rect::default();

        if area.is_empty() {
            self.layout_mode = LayoutMode::Full;
            return;
        }

        const MIN_RIGHT_AREA_WIDTH: u16 = 34;
        const MIN_FULL_DISPLAY_HEIGHT: u16 = metrics_display::HEIGHT + usage_graph::MIN_HEIGHT + 4;

        // Get the area for the log block
        let mut log_block_area = area;
        log_block_area.width = (area.width + 1) / 2;

        let mut right_area = Rect::new(log_block_area.right(), area.y, area.width - log_block_area.width, area.height);

        // Add one space of horizontal padding
        right_area.x += 1;
        right_area.width = right_area.width.saturating_sub(1);
        if right_area.width < MIN_RIGHT_AREA_WIDTH || right_area.height < usage_graph::MIN_HEIGHT + 3 {
            // Don't show the graph nor metrics, show just the log block with the expand graph label above it.
            let mut remaining_height = area.height;
            self.client_activity_line_area = Rect::new(area.x, area.y, area.width, remaining_height.min(1));
            if !self.client_activity_line_area.is_empty() {
                // Center the client activity line in the available width
                let (line_width, _) = self.client_activity_line.begin_resize(area.width, 1);
                let extra_width = area.width.saturating_sub(line_width);
                self.client_activity_line_area.x += extra_width / 2;
                self.client_activity_line_area.width -= extra_width;
            }

            remaining_height = remaining_height.saturating_sub(2);
            self.log_block_area = Rect::new(area.x, area.y + 2, area.width, remaining_height);
            self.layout_mode = LayoutMode::LogsOnly;
        } else {
            self.log_block_area = log_block_area;
            let mut remaining_height = right_area.height;

            // If there's enough space for the metrics display, then it is (and thefore all UI elements are) shown.
            // Otherwise, the metrics display isn't shown, only the usage graph and the labels on top of it.
            if right_area.width >= metrics_display::MIN_WIDTH && right_area.height >= MIN_FULL_DISPLAY_HEIGHT {
                let metrics_area_y = right_area.bottom() - metrics_display::HEIGHT;
                self.metrics_display_area = Rect::new(right_area.x, metrics_area_y, right_area.width, metrics_display::HEIGHT);
                remaining_height -= metrics_display::HEIGHT + 1;
            }

            self.client_activity_line_area = Rect::new(right_area.x, right_area.y, right_area.width, remaining_height.min(1));
            remaining_height -= 1;
            self.graph_precision_line_area = Rect::new(right_area.x, right_area.y + 1, right_area.width, remaining_height.min(1));
            remaining_height -= 2;
            self.usage_graph_area = Rect::new(right_area.x, right_area.y + 3, right_area.width, remaining_height);

            // Offset both labels a bit to look aligned with the graph's vertical axis
            let mut labels_offset_x = usage_graph::VERTICAL_LABELS_AREA_WIDTH;

            let (width, height) = (self.client_activity_line_area.width, self.client_activity_line_area.height);
            let (line_width, _) = self.client_activity_line.begin_resize(width, height);
            let extra_width = width.saturating_sub(line_width);
            self.client_activity_line_area.width -= extra_width;
            labels_offset_x = labels_offset_x.min(extra_width);

            let (width, height) = (self.graph_precision_line_area.width, self.graph_precision_line_area.height);
            let (line_width, _) = self.graph_precision_line.begin_resize(width, height);
            let extra_width = width.saturating_sub(line_width);
            self.graph_precision_line_area.width -= extra_width;
            labels_offset_x = labels_offset_x.min(extra_width);

            self.graph_precision_line_area.x += labels_offset_x;
            self.client_activity_line_area.x += labels_offset_x;

            self.layout_mode = LayoutMode::Full;
        }
    }

    fn distribute_areas_expanded(&mut self, area: Rect) {
        self.log_block_area = Rect::default();
        self.usage_graph_area = Rect::default();
        self.metrics_display_area = Rect::default();
        self.client_activity_line_area = Rect::default();
        self.graph_precision_line_area = Rect::default();

        self.layout_mode = LayoutMode::GraphExpanded;

        if area.height == 0 || area.width < usage_graph::VERTICAL_LABELS_AREA_WIDTH + 4 {
            return;
        }

        // Apply horizontal padding, 1 on each side
        let area = Rect::new(area.x + 1, area.y, area.width - 2, area.height);

        let mut top_line = Rect::new(area.x, area.y, area.width, 1);

        let (client_activity_line_width, _) = self.client_activity_line.begin_resize(top_line.width, 1);
        let client_activity_line_width = top_line.width.min(client_activity_line_width);
        self.client_activity_line_area = Rect::new(top_line.x, top_line.y, client_activity_line_width, 1);

        top_line.x += client_activity_line_width + 3;
        top_line.width = top_line.width.saturating_sub(client_activity_line_width + 3);
        self.graph_precision_line_area = top_line;

        self.usage_graph_area = Rect::new(area.x, area.y + 2, area.width, area.height.saturating_sub(2));
    }

    fn perform_resize(&mut self, area: Rect, expanded: bool) {
        match expanded {
            true => self.distribute_areas_expanded(area),
            false => self.distribute_areas(area),
        }

        if !self.log_block_area.is_empty() {
            self.log_block.resize(self.log_block_area);
        }

        if !self.usage_graph_area.is_empty() {
            self.usage_graph.resize(self.usage_graph_area);
        }

        if !self.metrics_display_area.is_empty() {
            self.metrics_display.resize(self.metrics_display_area);
        }

        if !self.client_activity_line_area.is_empty() {
            self.client_activity_line.resize(self.client_activity_line_area);
        }

        if !self.graph_precision_line_area.is_empty() {
            self.graph_precision_line.resize(self.graph_precision_line_area);
        }

        if self.is_focused && !self.focused_element.is_visible_in(self.layout_mode) {
            self.focus_lost();
            self.receive_focus((0, 0));
        }
    }
}

impl UIElement for MainView {
    fn resize(&mut self, area: Rect) {
        let expanded = self.controller.inner.borrow().expanded;
        self.perform_resize(area, expanded)
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let expanded = self.controller.inner.borrow().expanded;
        let is_layout_expanded = self.layout_mode == LayoutMode::GraphExpanded;
        if expanded != is_layout_expanded {
            if expanded {
                self.client_activity_line.right.set_text_no_redraw(RETURN_TO_MAIN_VIEW_LABEL.into());
                self.client_activity_line.right.set_shortcut(RETURN_TO_MAIN_VIEW_SHORTCUT);
            } else {
                self.client_activity_line.right.set_text_no_redraw(EXPAND_GRAPH_LABEL.into());
                self.client_activity_line.right.set_shortcut(EXPAND_GRAPH_SHORTCUT);
            }

            self.perform_resize(area, expanded);
        }

        if !self.log_block_area.is_empty() {
            self.log_block.render(self.log_block_area, frame);
        }

        if !self.usage_graph_area.is_empty() {
            self.usage_graph.render(self.usage_graph_area, frame);
        }

        if !self.metrics_display_area.is_empty() {
            self.metrics_display.render(self.metrics_display_area, frame);
        }

        if !self.client_activity_line_area.is_empty() {
            self.client_activity_line.render(self.client_activity_line_area, frame);
            if expanded {
                let x = self.client_activity_line_area.right() + 1;
                if x < area.right() {
                    let y = self.client_activity_line_area.y;
                    frame.buffer_mut().get_mut(x, y).set_char('|');
                }
            }
        }

        if !self.graph_precision_line_area.is_empty() {
            self.graph_precision_line.render(self.graph_precision_line_area, frame);
        }
    }

    fn handle_event(&mut self, event: &event::Event, is_focused: bool) -> HandleEventStatus {
        if is_focused {
            let status = match self.focused_element {
                FocusedElement::LogBlock => self.log_block.handle_event(event, true),
                FocusedElement::ExpandButton => self.client_activity_line.handle_event(event, true),
                FocusedElement::PrecisionSelector => self.graph_precision_line.handle_event(event, true),
            };

            match status {
                HandleEventStatus::Handled => return HandleEventStatus::Handled,
                HandleEventStatus::PassFocus(focus_position, direction) => {
                    let mut try_focused_element = self.focused_element;

                    loop {
                        let next_focused_element = match direction {
                            PassFocusDirection::Up => try_focused_element.up(self.layout_mode),
                            PassFocusDirection::Down => try_focused_element.down(self.layout_mode),
                            PassFocusDirection::Left => try_focused_element.left(self.layout_mode),
                            PassFocusDirection::Right => try_focused_element.right(self.layout_mode),
                            PassFocusDirection::Forward => try_focused_element.forward(self.layout_mode),
                            PassFocusDirection::Away => None,
                        };

                        try_focused_element = match next_focused_element {
                            Some(ele) => ele,
                            None => return status,
                        };

                        let focus_passed = match try_focused_element {
                            FocusedElement::LogBlock => self.log_block.receive_focus(focus_position),
                            FocusedElement::ExpandButton => self.client_activity_line.receive_focus(focus_position),
                            FocusedElement::PrecisionSelector => self.graph_precision_line.receive_focus(focus_position),
                        };

                        if focus_passed {
                            match self.focused_element {
                                FocusedElement::LogBlock => self.log_block.focus_lost(),
                                FocusedElement::ExpandButton => self.client_activity_line.focus_lost(),
                                FocusedElement::PrecisionSelector => self.graph_precision_line.focus_lost(),
                            }

                            self.focused_element = try_focused_element;
                            return HandleEventStatus::Handled;
                        }
                    }
                }
                HandleEventStatus::Unhandled => {}
            }
        }

        self.log_block
            .handle_event(event, false)
            .or_else(|| self.client_activity_line.handle_event(event, false))
            .or_else(|| self.graph_precision_line.handle_event(event, false))
    }

    fn receive_focus(&mut self, focus_position: (u16, u16)) -> bool {
        self.is_focused = true;
        let mut receive_order = [FocusedElement::LogBlock; 3];

        let receive_order_count = match self.layout_mode {
            LayoutMode::Full => {
                let log_block_first = focus_position.0 <= self.log_block_area.right();
                let expand_button_first = focus_position.1 <= self.client_activity_line_area.y;

                let i = match log_block_first {
                    true => 1,
                    false => 0,
                };
                (receive_order[i], receive_order[i + 1]) = match expand_button_first {
                    true => (FocusedElement::ExpandButton, FocusedElement::PrecisionSelector),
                    false => (FocusedElement::PrecisionSelector, FocusedElement::ExpandButton),
                };

                3
            }
            LayoutMode::LogsOnly => {
                let log_block_first = focus_position.1 > self.log_block_area.y;

                let i = match log_block_first {
                    true => 1,
                    false => 0,
                };
                receive_order[i] = FocusedElement::ExpandButton;

                2
            }
            LayoutMode::GraphExpanded => {
                let return_button_first = focus_position.0 < self.client_activity_line_area.right() + 2;
                (receive_order[0], receive_order[1]) = match return_button_first {
                    true => (FocusedElement::ExpandButton, FocusedElement::PrecisionSelector),
                    false => (FocusedElement::PrecisionSelector, FocusedElement::ExpandButton),
                };

                2
            }
        };

        for ele in receive_order[0..receive_order_count].iter() {
            let focus_received = match *ele {
                FocusedElement::LogBlock => self.log_block.receive_focus(focus_position),
                FocusedElement::ExpandButton => self.client_activity_line.receive_focus(focus_position),
                FocusedElement::PrecisionSelector => self.graph_precision_line.receive_focus(focus_position),
            };

            if focus_received {
                self.focused_element = *ele;
                return true;
            }
        }

        self.is_focused = false;
        false
    }

    fn focus_lost(&mut self) {
        self.is_focused = false;
        match self.focused_element {
            FocusedElement::LogBlock => self.log_block.focus_lost(),
            FocusedElement::ExpandButton => self.client_activity_line.focus_lost(),
            FocusedElement::PrecisionSelector => self.graph_precision_line.focus_lost(),
        }
    }
}
