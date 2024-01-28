use std::rc::Rc;

use ratatui::Frame;
use tokio::sync::Notify;

use super::menu_bar::MenuBar;

pub struct UIManager {
    menu_bar: MenuBar,
}

impl UIManager {
    pub fn new(redraw_notify: Rc<Notify>) -> Self {
        Self {
            menu_bar: MenuBar::new(redraw_notify),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let mut menu_area = frame.size();
        menu_area.height = menu_area.height.min(2);
        frame.render_widget(self.menu_bar.as_widget(), menu_area);
    }
}
