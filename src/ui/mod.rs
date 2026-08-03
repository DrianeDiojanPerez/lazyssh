pub mod panels;
pub mod popups;
pub mod renderer;
pub mod session;
pub mod tabs;
pub mod toasts;

pub use renderer::render;

#[cfg(test)]
pub mod screenshot {
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    use crate::services::AppService;

    /// The rendered frame with its styles intact, for the times a test cares
    /// about the colour a cell was drawn in.
    pub fn buffer(app: &AppService, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| super::render(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

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
