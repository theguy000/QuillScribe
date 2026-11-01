//! The fakes an utterance is tested against.
//!
//! Shared by the delivery tests so each test file can read as the behaviour it
//! checks rather than as its setup: a fake window that records what would have
//! been typed, and a fake clipboard for copy mode.

use super::super::*;
use anyhow::Result;
use std::sync::{Arc, Mutex};

pub(super) const BOUND: &str = "win:1";
pub(super) const OTHER: &str = "win:2";

/// Stands in for the operating system: records what would have been typed
/// into the target document, and answers which window has focus.
#[derive(Clone, Default)]
pub(super) struct FakeSurface {
    pasted: Arc<Mutex<Vec<String>>>,
    focus: Arc<Mutex<Option<WindowId>>>,
    failures: Arc<Mutex<usize>>,
    /// Whether the focused window is still a live window.
    exists: Arc<Mutex<bool>>,
}

impl FakeSurface {
    pub(super) fn focused_on(window: &str) -> Self {
        Self {
            focus: Arc::new(Mutex::new(Some(WindowId::new(window)))),
            exists: Arc::new(Mutex::new(true)),
            ..Self::default()
        }
    }

    pub(super) fn focus_unknown() -> Self {
        Self::default()
    }

    /// The user clicks another window, or back onto this one.
    pub(super) fn focus_on(&self, window: &str) {
        *self.focus.lock().unwrap() = Some(WindowId::new(window));
    }

    /// The bound window is closed under the utterance.
    pub(super) fn close(&self) {
        *self.focus.lock().unwrap() = None;
        *self.exists.lock().unwrap() = false;
    }

    /// The target refuses the next `times` pastes, without changing focus.
    pub(super) fn fail_next(&self, times: usize) {
        *self.failures.lock().unwrap() = times;
    }

    pub(super) fn failing(mut self, times: usize) -> Self {
        self.failures = Arc::new(Mutex::new(times));
        self.exists = Arc::new(Mutex::new(true));
        self
    }

    pub(super) fn pasted(&self) -> Vec<String> {
        self.pasted.lock().unwrap().clone()
    }
}

impl DeliverySurface for FakeSurface {
    fn paste(&mut self, text: &str) -> Result<()> {
        let mut failures = self.failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            anyhow::bail!("the target refused the paste");
        }
        drop(failures);
        self.pasted.lock().unwrap().push(text.to_string());
        Ok(())
    }

    fn focused_window(&self) -> Option<WindowId> {
        self.focus.lock().unwrap().clone()
    }

    fn focused_window_exists(&self) -> bool {
        *self.exists.lock().unwrap()
    }
}

pub(super) fn session(
    ledger: &Arc<Mutex<DeliveryLedger>>,
    surface: &FakeSurface,
    window: &str,
) -> DeliverySession {
    session_on(ledger, surface, window, TargetWindow::text_field())
}

/// The same, with what the platform said about the target.
pub(super) fn session_on(
    ledger: &Arc<Mutex<DeliveryLedger>>,
    surface: &FakeSurface,
    window: &str,
    target: TargetWindow,
) -> DeliverySession {
    DeliverySession::new(
        WindowId::new(window),
        target,
        surface.clone(),
        Arc::clone(ledger),
    )
}

/// The copy-mode target: the clipboard itself. There is no window to bind, so
/// `focus` answers nothing and no paste is ever asked for.
#[derive(Clone, Default)]
pub(super) struct FakeClipboard {
    pub(super) text: Arc<Mutex<Option<String>>>,
    pub(super) failures: Arc<Mutex<usize>>,
}

impl FakeClipboard {
    pub(super) fn text(&self) -> Option<String> {
        self.text.lock().unwrap().clone()
    }
}

impl DeliverySurface for FakeClipboard {
    fn paste(&mut self, _text: &str) -> Result<()> {
        anyhow::bail!("copy mode never pastes")
    }

    fn focused_window(&self) -> Option<WindowId> {
        None
    }

    fn focused_window_exists(&self) -> bool {
        false
    }

    fn replace_clipboard(&mut self, text: &str) -> Result<()> {
        let mut failures = self.failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            anyhow::bail!("the clipboard refused the write");
        }
        drop(failures);
        *self.text.lock().unwrap() = Some(text.to_string());
        Ok(())
    }
}

pub(super) fn copy_session(surface: &FakeClipboard) -> DeliverySession {
    DeliverySession::copy_to_clipboard(surface.clone())
}
