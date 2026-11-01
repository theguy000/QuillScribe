//! What live typing can do with the settings as they stand.
//!
//! The settings screen and the recording path both need the same answer to
//! "would text be typed while the user speaks?". The screen needs it to decide
//! whether its control is live, checked, and what to say when it is not; the
//! hotkey needs it to decide whether to bind a window at all. Two independent
//! readings would drift, and a control that looks on while nothing is typed is
//! the drift users notice first, so both read [`live_typing_capability`] and
//! nothing re-derives the conditions.
//!
//! The function is pure: it takes a [`Settings`] snapshot and returns a value.
//! No window, no provider, no socket.

use crate::config::Settings;
use crate::output::OutputMode;

/// How a live utterance would reach the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveDelivery {
    /// Each Final segment is pasted into the bound window.
    Type,
    /// There is no window to paste into, so the clipboard is rewritten after
    /// every Final segment with the transcript so far.
    CopyToClipboard,
}

/// What live typing can do with these settings, and why not when it cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveTypingCapability {
    /// Whether an utterance would type or copy while the user speaks.
    pub active: bool,
    /// Whether the app has a switch to show at all. Only Deepgram has a Live
    /// session to type from, so the control exists only in its group.
    pub control_available: bool,
    /// Whether that switch can be operated right now.
    pub control_enabled: bool,
    /// The short reason a disabled switch shows. `None` when it is operable.
    pub reason: Option<&'static str>,
    /// What an active utterance does. Meaningful only when `active`.
    pub delivery: LiveDelivery,
}

/// The screen's view of one [`LiveTypingCapability`].
///
/// The screen only ever displays this, so a switch cannot look on while the
/// recording path thinks it is off.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveTypingScreen {
    /// Whether to draw the switch at all.
    pub visible: bool,
    /// Whether tapping the switch does anything.
    pub enabled: bool,
    /// Whether it is drawn checked. A stored switch that cannot act, because
    /// the mode or the output mode rules it out, is still drawn checked: the
    /// user asked for it, and the reason says why nothing is typed.
    pub switch_checked: bool,
    /// The reason under a disabled switch.
    pub reason: Option<&'static str>,
    /// What an active switch does, in one line: typed into the focused
    /// application, or copied to the clipboard.
    pub description: &'static str,
}

impl LiveTypingCapability {
    /// The screen's reading, the only thing the screen displays.
    pub fn screen(&self, switch_checked: bool) -> LiveTypingScreen {
        LiveTypingScreen {
            visible: self.control_available,
            enabled: self.control_enabled,
            switch_checked: switch_checked && self.control_available,
            reason: self.reason,
            description: match self.delivery {
                LiveDelivery::Type => {
                    "Each finished phrase is typed into the app you are in, about a second after you pause."
                }
                LiveDelivery::CopyToClipboard => {
                    "Copy only: each finished phrase updates the clipboard, so a paste has everything so far."
                }
            },
        }
    }
}

/// Whether live typing would run for the next utterance.
///
/// This is what the hotkey asks before it binds a window, and it is the same
/// question the screen asks, so the two cannot disagree.
pub fn live_typing_capability(settings: &Settings) -> LiveTypingCapability {
    let control_available = settings.whisper.mode == "api"
        && crate::whisper::WhisperManager::is_live_provider(&settings.whisper.api_provider);
    let delivery = delivery_for(OutputMode::from(settings.output.mode));

    let reason = if !control_available {
        Some("Live typing is only available with Deepgram")
    } else if delivery.is_none() {
        Some("Display only mode keeps text inside QuillScribe")
    } else if !settings.deepgram.is_live() {
        Some("Live typing needs the live Deepgram mode")
    } else {
        None
    };

    let active = reason.is_none() && settings.deepgram.live_typing;
    LiveTypingCapability {
        active,
        control_available,
        control_enabled: reason.is_none() && control_available,
        reason,
        delivery: delivery.unwrap_or(LiveDelivery::CopyToClipboard),
    }
}

/// The delivery an output mode gives live typing, or `None` for a mode that
/// deliberately touches nothing outside the app.
fn delivery_for(mode: OutputMode) -> Option<LiveDelivery> {
    match mode {
        OutputMode::TypeToApp | OutputMode::CopyAndType => Some(LiveDelivery::Type),
        OutputMode::CopyToClipboard => Some(LiveDelivery::CopyToClipboard),
        OutputMode::DisplayOnly => None,
    }
}

/// Whether an utterance would type or copy while the user speaks.
///
/// The recording path calls this with the values it reads from settings. It is
/// written as a plain conjunction so a caller without a [`Settings`] snapshot
/// can still ask, and [`live_typing_capability`] is the only place the answer
/// is derived.
pub fn live_typing_applies(settings: &Settings) -> bool {
    live_typing_capability(settings).active
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether the screen shows a switch a user can turn on to make text appear:
    /// visible, operable, and drawn checked.
    fn shows_checked_switch(screen: LiveTypingScreen) -> bool {
        screen.visible && screen.enabled && screen.switch_checked
    }

    #[test]
    fn a_batch_provider_has_no_switch() {
        let mut settings = Settings::default();
        settings.whisper.api_provider = "openai".to_string();

        let screen = live_typing_capability(&settings).screen(true);

        assert!(!screen.visible);
        assert!(!shows_checked_switch(screen));
    }
}
