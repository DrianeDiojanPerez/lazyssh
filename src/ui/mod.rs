pub mod panels;
pub mod popups;
pub mod renderer;
pub mod toasts;

pub use renderer::render;

#[cfg(test)]
pub mod screenshot {
    use ratatui::{backend::TestBackend, Terminal};

    use crate::services::AppService;

    /// Renders the whole UI into a plain string so layout can be asserted on
    /// (and eyeballed with `cargo test -- --nocapture`).
    pub fn draw(app: &AppService, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| super::render(frame, app)).unwrap();

        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer.get(x, y).symbol());
            }
            out.push('\n');
        }
        out
    }
}
