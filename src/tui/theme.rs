use ratatui::style::Color;

pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    /// Highlights: tabs, selection accents, day headers.
    pub accent: Color,
    pub yellow: Color,
    pub green: Color,
    pub red: Color,
    pub selection_bg: Color,
}

pub const NORD: Theme = Theme {
    name: "nord",
    bg: Color::Rgb(0x2E, 0x34, 0x40),
    fg: Color::Rgb(0xD8, 0xDE, 0xE9),
    dim: Color::Rgb(0x6C, 0x77, 0x93),
    accent: Color::Rgb(0x88, 0xC0, 0xD0),
    yellow: Color::Rgb(0xEB, 0xCB, 0x8B),
    green: Color::Rgb(0xA3, 0xBE, 0x8C),
    red: Color::Rgb(0xBF, 0x61, 0x6A),
    selection_bg: Color::Rgb(0x43, 0x4C, 0x5E),
};

pub const DRACULA: Theme = Theme {
    name: "dracula",
    bg: Color::Rgb(0x28, 0x2A, 0x36),
    fg: Color::Rgb(0xF8, 0xF8, 0xF2),
    dim: Color::Rgb(0x62, 0x72, 0xA4),
    accent: Color::Rgb(0xBD, 0x93, 0xF9),
    yellow: Color::Rgb(0xF1, 0xFA, 0x8C),
    green: Color::Rgb(0x50, 0xFA, 0x7B),
    red: Color::Rgb(0xFF, 0x55, 0x55),
    selection_bg: Color::Rgb(0x44, 0x47, 0x5A),
};

pub const GRUVBOX: Theme = Theme {
    name: "gruvbox",
    bg: Color::Rgb(0x28, 0x28, 0x28),
    fg: Color::Rgb(0xEB, 0xDB, 0xB2),
    dim: Color::Rgb(0x92, 0x83, 0x74),
    accent: Color::Rgb(0x83, 0xA5, 0x98),
    yellow: Color::Rgb(0xFA, 0xBD, 0x2F),
    green: Color::Rgb(0xB8, 0xBB, 0x26),
    red: Color::Rgb(0xFB, 0x49, 0x34),
    selection_bg: Color::Rgb(0x50, 0x49, 0x45),
};

/// Uses the terminal's own palette — for terminals without truecolor.
pub const TERMINAL: Theme = Theme {
    name: "terminal",
    bg: Color::Reset,
    fg: Color::Reset,
    dim: Color::DarkGray,
    accent: Color::Cyan,
    yellow: Color::Yellow,
    green: Color::Green,
    red: Color::Red,
    selection_bg: Color::DarkGray,
};

pub const THEMES: &[&Theme] = &[&NORD, &DRACULA, &GRUVBOX, &TERMINAL];

pub fn by_name(name: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.name == name).copied().unwrap_or(&NORD)
}

pub fn next(current: &Theme) -> &'static Theme {
    let idx = THEMES.iter().position(|t| t.name == current.name).unwrap_or(0);
    THEMES[(idx + 1) % THEMES.len()]
}
