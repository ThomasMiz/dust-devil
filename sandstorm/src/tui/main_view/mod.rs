use std::rc::Rc;

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
const PRECISION_SELECTOR_AFTER_TEXT: &str = "[change (p)]";
const PRECISION_SELECTOR_SHORTCUT_KEY: char = 'p';

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
    current_area: Rect,
    log_block: LogBlock,
    log_block_area: Rect,
    usage_graph: UsageGraph,
    usage_graph_area: Rect,
    metrics_display: MetricsDisplay,
    metrics_display_area: Rect,
    client_activity_line: HorizontalSplit<TextLine, CenteredButton<StuffHandler>>,
    client_activity_line_area: Rect,
    graph_precision_line: HorizontalSplit<TextLine, ArrowSelector<StuffHandler>>,
    graph_precision_line_area: Rect,
    layout_mode: LayoutMode,
    focused_element: FocusedElement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Full,
    LogsOnly,
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
        }
    }
}

struct StuffHandler {}

impl StuffHandler {
    fn new() -> Self {
        Self {}
    }
}

impl ButtonHandler for StuffHandler {
    fn on_pressed(&mut self) -> OnEnterResult {
        OnEnterResult::Unhandled
    }
}

impl ArrowSelectorHandler for StuffHandler {
    fn selection_changed(&mut self, selected_index: usize) {}
}

impl MainView {
    pub fn new(redraw_notify: Rc<Notify>, metrics: Metrics) -> Self {
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
            StuffHandler::new(),
        );

        let client_activity_line = HorizontalSplit::new(client_activity_label, expand_graph_button, CLIENT_ACTIVITY_LABEL.len() as u16, 1);

        let graph_precision_label = TextLine::new(GRAPH_PRECISION_LABEL.into(), text_style, Alignment::Left);
        let graph_precision_selector = ArrowSelector::new(
            Rc::clone(&redraw_notify),
            ALL_GRAPH_PRECISION_OPTIONS.iter().map(|x| (x.to_str().into(), None)).collect(),
            0,
            text_style,
            selected_text_style,
            selected_text_style,
            selected_text_style,
            PRECISION_SELECTOR_AFTER_TEXT.into(),
            false,
            StuffHandler::new(),
        );

        let graph_precision_line = HorizontalSplit::new(
            graph_precision_label,
            graph_precision_selector,
            GRAPH_PRECISION_LABEL.len() as u16,
            1,
        );

        Self {
            current_area: Rect::default(),
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
            layout_mode: LayoutMode::Full,
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
        if area == self.current_area {
            return;
        }

        self.log_block_area = Rect::default();
        self.usage_graph_area = Rect::default();
        self.metrics_display_area = Rect::default();
        self.client_activity_line_area = Rect::default();
        self.graph_precision_line_area = Rect::default();

        if area.is_empty() {
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
}

impl UIElement for MainView {
    fn resize(&mut self, area: Rect) {
        self.distribute_areas(area);

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

        if !self.focused_element.is_visible_in(self.layout_mode) {
            self.focus_lost();
            self.receive_focus((0, 0));
        }
    }

    fn render(&mut self, _area: Rect, frame: &mut Frame) {
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

        false
    }

    fn focus_lost(&mut self) {
        match self.focused_element {
            FocusedElement::LogBlock => self.log_block.focus_lost(),
            FocusedElement::ExpandButton => self.client_activity_line.focus_lost(),
            FocusedElement::PrecisionSelector => self.graph_precision_line.focus_lost(),
        }
    }
}
