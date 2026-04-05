use ratatui::{Frame, layout::Rect, widgets::Paragraph};

pub struct ScrollableList<'a> {
    items: Vec<ratatui::text::Line<'a>>,
    scroll: usize,
    height: usize,
}

impl<'a> ScrollableList<'a> {
    pub fn new(items: Vec<ratatui::text::Line<'a>>) -> Self {
        Self {
            items,
            scroll: 0,
            height: 0,
        }
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = self.items.len().saturating_sub(self.height);
        self.scroll = (self.scroll + 1).min(max_scroll);
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_to_bottom(&mut self) {
        let max_scroll = self.items.len().saturating_sub(self.height);
        self.scroll = max_scroll;
    }

    pub fn update_height(&mut self, area: Rect) {
        self.height = area.height.saturating_sub(2) as usize;
    }
}

pub struct StatusBar {
    pub left: String,
    pub center: String,
    pub right: String,
}

impl StatusBar {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let line = ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!(" {} ", self.left),
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Rgb(15, 15, 20))
                    .bg(ratatui::style::Color::Rgb(120, 180, 255)),
            ),
            ratatui::text::Span::raw(" "),
            ratatui::text::Span::styled(
                &self.center,
                ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(120, 120, 140)),
            ),
            ratatui::text::Span::raw(" "),
            ratatui::text::Span::styled(
                format!(" {} ", self.right),
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Rgb(15, 15, 20))
                    .bg(ratatui::style::Color::Rgb(120, 180, 255)),
            ),
        ]);

        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}

pub struct CodeBlock {
    pub content: String,
    pub language: Option<String>,
}

impl CodeBlock {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let paragraph = Paragraph::new(self.content.as_str());
        frame.render_widget(paragraph, area);
    }
}
}
