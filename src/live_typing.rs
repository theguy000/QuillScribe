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
use crate::output::{focus_reading_unavailable, OutputMode};

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
    /// A machine that cannot read the focused window puts its reason here too,
    /// alongside the settings that rule live typing out.
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
    ///
    /// The switch is drawn checked only when it is operable, so an in-memory
    /// pair that the settings file would have corrected cannot make it claim
    /// to be on while the recording path reads it as off.
    pub fn screen(&self) -> LiveTypingScreen {
        LiveTypingScreen {
            visible: self.control_available,
            enabled: self.control_enabled,
            switch_checked: self.active,
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
/// question the screen asks, so the two cannot disagree. A recorded mode with
/// the toggle on is read as off, the same way a settings file that pairs them
/// loads, so no caller has to remember to reconcile first. `active` says the
/// settings allow live typing; `unavailable` says this machine cannot do it.
pub fn live_typing_capability(settings: &Settings) -> LiveTypingCapability {
    live_typing_capability_on(settings, focus_reading_unavailable())
}

/// The capability, given what this machine's platform said about reading the
/// focused window.
///
/// Kept separate so the Linux answers can be checked on any build, including
/// the Windows one the tests usually run on. A machine reason disables the
/// switch the same way a display-only output does, because the same thing would
/// happen: no text types. It says nothing about the stored setting, so the
/// choice is intact if the machine can type again. Copy mode never reads a
/// window, so it ignores the answer entirely.
pub fn live_typing_capability_on(
    settings: &Settings,
    machine: Option<&'static str>,
) -> LiveTypingCapability {
    // Only a provider whose live session commits segments while the user speaks
    // can type as they speak. Gemini streams audio but produces its transcript at
    // the end, so it has no switch here.
    let control_available = settings.whisper.mode == "api"
        && crate::whisper::WhisperManager::supports_live_typing(&settings.whisper.api_provider);
    let delivery = delivery_for(OutputMode::from(settings.output.mode));
    let live = settings.deepgram.is_live() && settings.deepgram.live_typing;

    let reason = match delivery {
        // Copy mode never reads a window, so what the machine says about focus
        // reading does not apply to it.
        Some(LiveDelivery::Type) => settings_reason(settings).or(machine),
        _ => settings_reason(settings),
    };

    let enabled = reason.is_none();
    LiveTypingCapability {
        // A disabled switch cannot act, whatever the stored setting says, so
        // the recording path and the screen read the same answer.
        active: enabled && live,
        control_available,
        control_enabled: enabled,
        reason,
        delivery: delivery.unwrap_or(LiveDelivery::CopyToClipboard),
    }
}

/// Why the settings alone rule live typing out, or `None` when they allow it.
///
/// Each answer matches what a recording would do. Only Deepgram's live session
/// commits segments while the user speaks, so only it has something to type.
/// Display only writes nothing outside the app. With no key the recording path
/// stops the utterance before it starts, and a recorded session sends its text
/// at stop.
fn settings_reason(settings: &Settings) -> Option<&'static str> {
    let commits_segments = settings.whisper.mode == "api"
        && crate::whisper::WhisperManager::supports_live_typing(&settings.whisper.api_provider);
    if !commits_segments {
        Some("Live typing is only available with Deepgram")
    } else if OutputMode::from(settings.output.mode) == OutputMode::DisplayOnly {
        Some("Display only mode keeps text inside QuillScribe")
    } else if settings.deepgram.api_key.trim().is_empty() {
        Some("Add a Deepgram API key to type as you speak")
    } else if !settings.deepgram.is_live() {
        Some("Live typing needs the live Deepgram mode")
    } else {
        None
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

        let screen = live_typing_capability(&settings).screen();

        assert!(!screen.visible);
        assert!(!shows_checked_switch(screen));
    }

    #[test]
    fn a_type_delivery_says_so_under_the_switch() {
        // The hint under an active switch has to describe what actually
        // happens: with live typing on, text arrives during the utterance, not
        // at stop.
        let mut settings = Settings::default();
        settings.whisper.api_provider = "deepgram".to_string();
        settings.deepgram.api_key = "not-a-real-deepgram-key".to_string();

        let screen = live_typing_capability(&settings).screen();
        assert!(screen.description.contains("typed into the app you are in"));
    }

    /// Live typing ready to run, as a fresh install has it.
    fn ready_settings() -> Settings {
        let mut settings = Settings::default();
        settings.whisper.api_provider = "deepgram".to_string();
        settings.deepgram.api_key = "not-a-real-deepgram-key".to_string();
        settings
    }

    #[test]
    fn a_ready_machine_reports_no_machine_reason() {
        let capability = live_typing_capability_on(&ready_settings(), None);

        assert!(capability.active);
        assert_eq!(capability.reason, None);
        assert!(shows_checked_switch(capability.screen()));
    }

    #[test]
    fn a_machine_that_cannot_read_focus_disables_the_switch_and_says_why() {
        // Wayland and X11 without xdotool both land here: the recording path
        // creates no delivery session, so no sentence arrives before stop.
        let reason = "Wayland does not let an app read which window has focus";
        let capability = live_typing_capability_on(&ready_settings(), Some(reason));

        assert!(!capability.active, "a disabled switch cannot act");
        assert_eq!(capability.reason, Some(reason));

        // The screen draws the reason only under a disabled switch, so the
        // switch has to be disabled for the reason to be visible at all.
        let screen = capability.screen();
        assert!(screen.visible);
        assert!(!screen.enabled);
        assert!(!screen.switch_checked);
        assert_eq!(screen.reason, Some(reason));
    }

    #[test]
    fn a_copy_only_utterance_still_runs_on_a_machine_that_cannot_read_focus() {
        // Copy mode never binds a window, so a Wayland session still copies
        // after every Final segment and has no reason to show.
        let mut settings = ready_settings();
        settings.output.mode = 0;

        let capability = live_typing_capability_on(&settings, Some("Wayland"));

        assert!(capability.active);
        assert_eq!(capability.reason, None);
        assert_eq!(capability.delivery, LiveDelivery::CopyToClipboard);
        assert!(shows_checked_switch(capability.screen()));
    }

    #[test]
    fn the_settings_reason_wins_over_the_machine_reason() {
        // Recording mode already explains that live typing cannot run; a second
        // reason under the same switch would only compete with it.
        let mut settings = ready_settings();
        settings.deepgram.set_mode("recorded");
        let capability = live_typing_capability_on(&settings, Some("Wayland"));

        assert_eq!(
            capability.reason,
            Some("Live typing needs the live Deepgram mode")
        );
    }
}
