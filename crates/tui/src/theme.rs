use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub surface: Color,
    pub border: Color,
    pub border_active: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_inverse: Color,
    pub primary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            background: Color::Rgb(15, 15, 20),
            surface: Color::Rgb(25, 25, 35),
            border: Color::Rgb(50, 50, 65),
            border_active: Color::Rgb(120, 180, 255),
            text: Color::Rgb(220, 220, 230),
            text_muted: Color::Rgb(120, 120, 140),
            text_inverse: Color::Rgb(15, 15, 20),
            primary: Color::Rgb(120, 180, 255),
            success: Color::Rgb(80, 200, 120),
            warning: Color::Rgb(255, 180, 60),
            error: Color::Rgb(255, 90, 90),
            info: Color::Rgb(100, 160, 250),
        }
    }

    pub fn light() -> Self {
        Self {
            background: Color::Rgb(245, 245, 250),
            surface: Color::Rgb(255, 255, 255),
            border: Color::Rgb(200, 200, 210),
            border_active: Color::Rgb(60, 120, 200),
            text: Color::Rgb(30, 30, 40),
            text_muted: Color::Rgb(130, 130, 150),
            text_inverse: Color::Rgb(255, 255, 255),
            primary: Color::Rgb(40, 100, 200),
            success: Color::Rgb(30, 140, 80),
            warning: Color::Rgb(200, 140, 30),
            error: Color::Rgb(200, 50, 50),
            info: Color::Rgb(50, 110, 200),
        }
    }

    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    pub fn primary_style(&self) -> Style {
        Style::default().fg(self.primary)
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn info_style(&self) -> Style {
        Style::default().fg(self.info)
    }

    pub fn bold_style(&self) -> Style {
        Style::default().fg(self.text).add_modifier(Modifier::BOLD)
    }

    pub fn inverse_style(&self) -> Style {
        Style::default()
            .fg(self.text_inverse)
            .bg(self.primary)
            .add_modifier(Modifier::BOLD)
    }
}
