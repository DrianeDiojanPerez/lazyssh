use std::time::Duration;

use chrono::Local;

/// How long a toast takes to slide in, and to slide back out again.
const ENTER: Duration = Duration::from_millis(200);
const LEAVE: Duration = Duration::from_millis(260);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastKind {
    Success,
    Error,
}

impl ToastKind {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Error => "Error",
        }
    }
}

/// A message that shows itself in the top right corner and leaves on its own.
/// It carries its own age rather than a timestamp, so the render loop decides
/// how time passes and the tests can hand it any amount they like.
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    /// The wall clock time it was raised, shown on the title row.
    pub at: String,
    age: Duration,
}

impl Toast {
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Success)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Error)
    }

    fn new(message: impl Into<String>, kind: ToastKind) -> Self {
        Self {
            message: message.into(),
            kind,
            at: Local::now().format("%H:%M:%S").to_string(),
            age: Duration::ZERO,
        }
    }

    /// Errors are worth reading twice, so they stay around longer.
    fn lifetime(&self) -> Duration {
        match self.kind {
            ToastKind::Success => Duration::from_millis(5000),
            ToastKind::Error => Duration::from_millis(8000),
        }
    }

    pub fn advance(&mut self, delta: Duration) {
        self.age += delta;
    }

    pub fn is_finished(&self) -> bool {
        self.age >= self.lifetime()
    }

    /// 0.0 when the toast is still off screen and 1.0 once it is fully open,
    /// eased at both ends so it glides instead of popping into place.
    pub fn openness(&self) -> f32 {
        let entering = ratio(self.age, ENTER);
        let leaving = ratio(self.lifetime().saturating_sub(self.age), LEAVE);

        ease_out(entering.min(leaving))
    }

    /// The share of its life still to run, drawn as the bar underneath.
    pub fn remaining(&self) -> f32 {
        1.0 - ratio(self.age, self.lifetime())
    }
}

fn ratio(part: Duration, whole: Duration) -> f32 {
    (part.as_secs_f32() / whole.as_secs_f32()).clamp(0.0, 1.0)
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aged(millis: u64) -> Toast {
        let mut toast = Toast::success("saved");
        toast.advance(Duration::from_millis(millis));
        toast
    }

    #[test]
    fn a_toast_slides_in_settles_and_slides_back_out() {
        assert_eq!(aged(0).openness(), 0.0);
        assert!(aged(80).openness() > 0.0 && aged(80).openness() < 1.0);
        assert_eq!(aged(1000).openness(), 1.0);
        assert!(aged(4900).openness() < 1.0);
        assert_eq!(aged(5000).openness(), 0.0);
    }

    #[test]
    fn a_toast_leaves_once_its_life_runs_out() {
        assert!(!aged(4999).is_finished());
        assert!(aged(5000).is_finished());

        let mut error = Toast::error("write failed");
        error.advance(Duration::from_millis(5000));
        assert!(!error.is_finished(), "errors are given longer to be read");
    }

    #[test]
    fn the_bar_underneath_empties_over_the_lifetime() {
        assert_eq!(aged(0).remaining(), 1.0);
        assert_eq!(aged(2500).remaining(), 0.5);
        assert_eq!(aged(5000).remaining(), 0.0);
    }
}
