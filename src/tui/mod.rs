mod app;
mod input;
mod theme;
mod ui;

use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::config::{Config, Ctx};
use app::App;

pub fn run() -> Result<()> {
    // Theme is known before any network happens — the splash matches it.
    let theme_name =
        Config::load().ok().and_then(|c| c.theme).unwrap_or_else(|| "nord".to_string());
    let theme = theme::by_name(&theme_name);

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, theme);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, theme: &'static theme::Theme) -> Result<()> {
    // Ctx::load talks to 1Password (and possibly the API) — do it behind a
    // splash screen instead of blocking before the first frame.
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(Ctx::load());
    });
    let ctx = loop {
        terminal.draw(|f| ui::draw_splash(f, theme))?;
        if event::poll(Duration::from_millis(150))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
        match rx.try_recv() {
            Ok(ctx) => break ctx?,
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => bail!("startup thread died"),
        }
    };

    let mut app = App::new(ctx)?;
    loop {
        app.pump();
        terminal.draw(|f| ui::draw(f, &app))?;
        // Short poll so the running timer's elapsed display stays live.
        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
        }
        if app.quit {
            return Ok(());
        }
    }
}
