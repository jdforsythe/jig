use ratatui::style::Color;

pub struct Theme {
    pub border_focused: Color,
    pub border_unfocused: Color,
    pub highlight_bg: Color,
    pub token_warn: Color,
    pub token_critical: Color,
    pub success: Color,
    pub failure: Color,
}

impl Theme {
    pub fn default_16color() -> Self {
        Self {
            border_focused: Color::Cyan,
            border_unfocused: Color::DarkGray,
            highlight_bg: Color::Cyan,
            token_warn: Color::Yellow,
            token_critical: Color::Red,
            success: Color::Green,
            failure: Color::Red,
        }
    }

    pub fn truecolor() -> Self {
        Self {
            border_focused: Color::Rgb(86, 182, 194),
            border_unfocused: Color::Rgb(80, 80, 80),
            highlight_bg: Color::Rgb(86, 182, 194),
            token_warn: Color::Rgb(229, 192, 123),
            token_critical: Color::Rgb(224, 108, 117),
            success: Color::Rgb(152, 195, 121),
            failure: Color::Rgb(224, 108, 117),
        }
    }
}

pub fn detect_truecolor() -> bool {
    matches!(
        std::env::var("COLORTERM").as_deref(),
        Ok("truecolor") | Ok("24bit")
    )
}

pub fn active_theme() -> Theme {
    if detect_truecolor() {
        Theme::truecolor()
    } else {
        Theme::default_16color()
    }
}
