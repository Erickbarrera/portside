//! Controller input for moving around the app (docs/ui.md, "Controller"). Reads every
//! connected gamepad with gilrs and sends `gamepad-input` events; the UI moves focus.
//! Read here rather than with the webview's Gamepad API, which WebKitGTK (Linux, Steam Deck)
//! doesn't reliably offer.

use std::time::{Duration, Instant};

use gilrs::{Axis, Button, Gilrs};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;

/// What a controller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PadAction {
    Up,
    Down,
    Left,
    Right,
    /// A (bottom face button): press the focused button or link.
    Accept,
    /// B (right face button): close a dialog, else go back.
    Back,
    /// Left bumper: the previous section in the sidebar.
    PrevSection,
    /// Right bumper: the next section.
    NextSection,
    /// Right stick: scroll the page.
    ScrollUp,
    ScrollDown,
}

/// Sent for each controller action, including repeats while a direction is held.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct GamepadInput {
    pub action: PadAction,
}

/// What is held down right now, across all controllers.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Held {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub accept: bool,
    pub back: bool,
    pub prev: bool,
    pub next: bool,
    /// Right stick, -1 (down) to 1 (up).
    pub scroll: f32,
}

/// A held direction repeats after this long…
const REPEAT_DELAY: Duration = Duration::from_millis(400);
/// …then this often.
const REPEAT_EVERY: Duration = Duration::from_millis(110);
/// How far a stick must lean to count as a direction.
const STICK_ON: f32 = 0.6;
/// How often the right stick scrolls when pushed all the way.
const SCROLL_EVERY: Duration = Duration::from_millis(50);

/// Turns what is held into actions: buttons fire once per press, directions and scrolling
/// repeat while held.
#[derive(Debug, Default)]
pub struct Nav {
    last: Held,
    /// The direction being held and when it next repeats.
    repeat: Option<(PadAction, Instant)>,
    next_scroll: Option<Instant>,
}

impl Nav {
    pub fn tick(&mut self, now: Instant, held: Held) -> Vec<PadAction> {
        let mut out = Vec::new();
        let presses = [
            (held.accept, self.last.accept, PadAction::Accept),
            (held.back, self.last.back, PadAction::Back),
            (held.prev, self.last.prev, PadAction::PrevSection),
            (held.next, self.last.next, PadAction::NextSection),
        ];
        for (down, was, action) in presses {
            if down && !was {
                out.push(action);
            }
        }

        // One direction at a time: a diagonal counts as the first of up, down, left, right.
        let dir = [
            (held.up, PadAction::Up),
            (held.down, PadAction::Down),
            (held.left, PadAction::Left),
            (held.right, PadAction::Right),
        ]
        .into_iter()
        .find_map(|(on, a)| on.then_some(a));
        self.repeat = match (dir, self.repeat) {
            (None, _) => None,
            (Some(d), Some((was, at))) if d == was => {
                if now >= at {
                    out.push(d);
                    Some((d, now + REPEAT_EVERY))
                } else {
                    Some((was, at))
                }
            }
            (Some(d), _) => {
                out.push(d);
                Some((d, now + REPEAT_DELAY))
            }
        };

        let lean = held.scroll.abs();
        if lean < 0.3 {
            self.next_scroll = None;
        } else if self.next_scroll.is_none_or(|at| now >= at) {
            out.push(if held.scroll > 0.0 {
                PadAction::ScrollUp
            } else {
                PadAction::ScrollDown
            });
            // Lean further to scroll faster.
            self.next_scroll = Some(now + SCROLL_EVERY.mul_f32(1.0 / lean));
        }

        self.last = held;
        out
    }

    /// True while something repeats, so input is checked often.
    fn busy(&self) -> bool {
        self.repeat.is_some() || self.next_scroll.is_some()
    }
}

fn held(gilrs: &Gilrs) -> Held {
    let mut h = Held::default();
    for (_, pad) in gilrs.gamepads() {
        let x = pad.value(Axis::LeftStickX);
        let y = pad.value(Axis::LeftStickY);
        h.up |= pad.is_pressed(Button::DPadUp) || y > STICK_ON;
        h.down |= pad.is_pressed(Button::DPadDown) || y < -STICK_ON;
        h.left |= pad.is_pressed(Button::DPadLeft) || x < -STICK_ON;
        h.right |= pad.is_pressed(Button::DPadRight) || x > STICK_ON;
        h.accept |= pad.is_pressed(Button::South);
        h.back |= pad.is_pressed(Button::East);
        h.prev |= pad.is_pressed(Button::LeftTrigger);
        h.next |= pad.is_pressed(Button::RightTrigger);
        let s = pad.value(Axis::RightStickY);
        if s.abs() > h.scroll.abs() {
            h.scroll = s;
        }
    }
    h
}

/// Reads controllers on their own thread for as long as the app runs. Without controller
/// support on this system it logs once and stops.
pub fn spawn(app: AppHandle) {
    let started = std::thread::Builder::new()
        .name("gamepad".into())
        .spawn(move || {
            let mut gilrs = match Gilrs::new() {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("Controllers unavailable: {e}");
                    return;
                }
            };
            let mut nav = Nav::default();
            loop {
                // Wait for input when idle; check often while a direction repeats.
                let wait = if nav.busy() {
                    Duration::from_millis(16)
                } else {
                    Duration::from_millis(250)
                };
                if gilrs.next_event_blocking(Some(wait)).is_some() {
                    while gilrs.next_event().is_some() {}
                }
                for action in nav.tick(Instant::now(), held(&gilrs)) {
                    let _ = GamepadInput { action }.emit(&app);
                }
            }
        });
    if let Err(e) = started {
        eprintln!("Controllers unavailable: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(start: Instant, n: u64) -> Instant {
        start + Duration::from_millis(n)
    }

    #[test]
    fn buttons_fire_once_per_press() {
        let t = Instant::now();
        let mut nav = Nav::default();
        let a = Held {
            accept: true,
            ..Held::default()
        };
        assert_eq!(nav.tick(t, a), vec![PadAction::Accept]);
        assert!(nav.tick(ms(t, 500), a).is_empty());
        assert!(nav.tick(ms(t, 600), Held::default()).is_empty());
        assert_eq!(nav.tick(ms(t, 700), a), vec![PadAction::Accept]);
    }

    #[test]
    fn held_direction_repeats_after_a_delay() {
        let t = Instant::now();
        let mut nav = Nav::default();
        let down = Held {
            down: true,
            ..Held::default()
        };
        assert_eq!(nav.tick(t, down), vec![PadAction::Down]);
        assert!(nav.tick(ms(t, 200), down).is_empty());
        assert_eq!(nav.tick(ms(t, 400), down), vec![PadAction::Down]);
        assert!(nav.tick(ms(t, 450), down).is_empty());
        assert_eq!(nav.tick(ms(t, 510), down), vec![PadAction::Down]);
        assert!(nav.tick(ms(t, 520), Held::default()).is_empty());
        assert!(!nav.busy());
    }

    #[test]
    fn changing_direction_moves_at_once() {
        let t = Instant::now();
        let mut nav = Nav::default();
        let up = Held {
            up: true,
            ..Held::default()
        };
        let right = Held {
            right: true,
            ..Held::default()
        };
        assert_eq!(nav.tick(t, up), vec![PadAction::Up]);
        assert_eq!(nav.tick(ms(t, 50), right), vec![PadAction::Right]);
    }

    #[test]
    fn right_stick_scrolls_faster_when_pushed_further() {
        let t = Instant::now();
        let mut nav = Nav::default();
        let full = Held {
            scroll: -1.0,
            ..Held::default()
        };
        assert_eq!(nav.tick(t, full), vec![PadAction::ScrollDown]);
        assert_eq!(nav.tick(ms(t, 50), full), vec![PadAction::ScrollDown]);
        let half = Held {
            scroll: 0.5,
            ..Held::default()
        };
        assert_eq!(nav.tick(ms(t, 100), half), vec![PadAction::ScrollUp]);
        assert!(nav.tick(ms(t, 150), half).is_empty());
        assert_eq!(nav.tick(ms(t, 200), half), vec![PadAction::ScrollUp]);
        assert!(nav.tick(ms(t, 210), Held::default()).is_empty());
    }
}
