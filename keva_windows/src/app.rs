//! Application coordinator.

use crate::webview::messages::OutgoingMessage;
use std::sync::mpsc::Receiver;

/// Application coordinator, owns state and services.
pub struct App {
    /// Receiver for keva worker responses.
    pub response_rx: Receiver<OutgoingMessage>,
}
