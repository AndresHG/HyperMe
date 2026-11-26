//! Paste at the cursor through the clipboard (R-15, R-16): save what the clipboard holds, put
//! the text on it, send the paste shortcut, and a moment later put the old contents back,
//! unless something else has changed the clipboard in the meantime.
//!
//! [`ClipboardInserter`] is pure and tested on every OS against fakes; on macOS,
//! [`macos`] provides the real pasteboard and the Cmd+V keystroke.

#[cfg(target_os = "macos")]
pub mod macos;

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use hyperme_core::traits::{Error, Inserter};

/// One clipboard item: each of its types with its data.
pub type Item = Vec<(String, Vec<u8>)>;

/// The system clipboard.
pub trait Pasteboard: Send + Sync + 'static {
    /// Goes up every time the clipboard's contents change.
    fn change_count(&self) -> i64;
    /// Every item on the clipboard, with all its types.
    fn save(&self) -> Vec<Item>;
    /// Replaces the contents with `text`; returns the change count after the write.
    fn write_text(&self, text: &str) -> Result<i64, Error>;
    /// Replaces the contents with `items` (none leaves the clipboard empty).
    fn restore(&self, items: &[Item]) -> Result<(), Error>;
}

/// Sends the paste shortcut to the frontmost app.
pub trait Keystrokes {
    fn paste_shortcut(&self) -> Result<(), Error>;
}

/// A restore waiting to happen.
struct Pending {
    saved: Vec<Item>,
    /// The change count right after our write; any other value means someone else wrote.
    ours: i64,
    generation: u64,
}

/// How long apps get to read the clipboard after the paste shortcut before it is restored.
/// Apps read it asynchronously; slow ones (Electron, browsers) need a few hundred ms.
pub const RESTORE_AFTER: Duration = Duration::from_millis(500);

/// An [`Inserter`] that pastes through the clipboard and restores it afterwards.
pub struct ClipboardInserter<P, K> {
    board: Arc<P>,
    keys: K,
    restore_after: Duration,
    pending: Arc<Mutex<Option<Pending>>>,
    generation: u64,
    last_restore: Option<JoinHandle<()>>,
}

impl<P, K> std::fmt::Debug for ClipboardInserter<P, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardInserter")
            .field("restore_after", &self.restore_after)
            .finish_non_exhaustive()
    }
}

impl<P: Pasteboard, K: Keystrokes> ClipboardInserter<P, K> {
    pub fn new(board: Arc<P>, keys: K, restore_after: Duration) -> Self {
        ClipboardInserter {
            board,
            keys,
            restore_after,
            pending: Arc::default(),
            generation: 0,
            last_restore: None,
        }
    }

    /// Blocks until the last scheduled restore has run or been skipped. For tests.
    pub fn wait_for_restore(&mut self) {
        if let Some(handle) = self.last_restore.take() {
            let _ = handle.join();
        }
    }

    fn paste_now(&mut self, text: &str) -> Result<(), Error> {
        // A restore still pending holds the user's own clipboard: keep that, not our last
        // dictation, unless the user has copied something since (R-16).
        let saved = {
            let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            match pending.take() {
                Some(p) if self.board.change_count() == p.ours => p.saved,
                _ => self.board.save(),
            }
        };
        let ours = self.board.write_text(text)?;
        self.generation += 1;
        let generation = self.generation;
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = Some(Pending {
            saved,
            ours,
            generation,
        });
        let sent = self.keys.paste_shortcut();

        let (board, pending, wait) = (self.board.clone(), self.pending.clone(), self.restore_after);
        self.last_restore = Some(std::thread::spawn(move || {
            std::thread::sleep(wait);
            let mut pending = pending.lock().unwrap_or_else(PoisonError::into_inner);
            // A later paste took over this restore.
            if pending.as_ref().is_none_or(|p| p.generation != generation) {
                return;
            }
            let Some(p) = pending.take() else { return };
            // AC-13: if anything else wrote to the clipboard, leave it alone.
            if board.change_count() == p.ours {
                let _ = board.restore(&p.saved);
            }
        }));
        sent
    }
}

impl<P: Pasteboard, K: Keystrokes + Send> Inserter for ClipboardInserter<P, K> {
    async fn paste(&mut self, text: &str) -> Result<(), Error> {
        self.paste_now(text)
    }

    /// R-31: the text stays on the clipboard; there is nothing to restore.
    async fn copy(&mut self, text: &str) -> Result<(), Error> {
        // A restore still pending would overwrite this copy.
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.board.write_text(text).map(|_| ())
    }
}
