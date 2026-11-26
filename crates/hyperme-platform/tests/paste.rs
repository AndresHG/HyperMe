//! Slice 8: paste at the cursor through the clipboard, and its restore. Tests cite the spec's
//! requirements and acceptance criteria (`specs/25-dictation-core/spec.md`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyperme_core::traits::{Error, Inserter};
use hyperme_platform::paste::{ClipboardInserter, Item, Keystrokes, Pasteboard};

/// A clipboard in memory: its items and macOS's change count.
#[derive(Default)]
struct FakeBoard {
    state: Mutex<(i64, Vec<Item>)>,
    log: Mutex<Vec<String>>,
}

impl FakeBoard {
    fn holding(text: &str) -> Arc<Self> {
        let board = Arc::new(FakeBoard::default());
        board.set(text);
        board
    }

    /// Another app copies something.
    fn set(&self, text: &str) {
        let mut state = self.state.lock().unwrap();
        state.0 += 1;
        state.1 = vec![vec![(
            "public.utf8-plain-text".to_owned(),
            text.as_bytes().to_vec(),
        )]];
    }

    fn text(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        let item = state.1.first()?;
        item.iter()
            .find(|(t, _)| t == "public.utf8-plain-text")
            .map(|(_, data)| String::from_utf8(data.clone()).unwrap())
    }

    fn items(&self) -> Vec<Item> {
        self.state.lock().unwrap().1.clone()
    }
}

impl Pasteboard for FakeBoard {
    fn change_count(&self) -> i64 {
        self.state.lock().unwrap().0
    }

    fn save(&self) -> Vec<Item> {
        self.items()
    }

    fn write_text(&self, text: &str) -> Result<i64, Error> {
        self.set(text);
        self.log.lock().unwrap().push(format!("write: {text}"));
        Ok(self.change_count())
    }

    fn restore(&self, items: &[Item]) -> Result<(), Error> {
        let mut state = self.state.lock().unwrap();
        state.0 += 1;
        state.1 = items.to_vec();
        self.log.lock().unwrap().push("restore".to_owned());
        Ok(())
    }
}

/// Records each paste shortcut with what the clipboard held at that moment.
struct FakeKeys {
    board: Arc<FakeBoard>,
    pasted: Pasted,
}

impl Keystrokes for FakeKeys {
    fn paste_shortcut(&self) -> Result<(), Error> {
        self.pasted.lock().unwrap().push(self.board.text());
        Ok(())
    }
}

/// What the clipboard held at each paste shortcut.
type Pasted = Arc<Mutex<Vec<Option<String>>>>;

const RESTORE_AFTER: Duration = Duration::from_millis(30);

fn inserter(board: &Arc<FakeBoard>) -> (ClipboardInserter<FakeBoard, FakeKeys>, Pasted) {
    let pasted = Arc::new(Mutex::new(Vec::new()));
    let keys = FakeKeys {
        board: board.clone(),
        pasted: pasted.clone(),
    };
    (
        ClipboardInserter::new(board.clone(), keys, RESTORE_AFTER),
        pasted,
    )
}

/// R-15, R-16: the text is put on the clipboard, the paste shortcut is sent, and afterwards
/// the clipboard gets back what it held, every item and type.
#[tokio::test]
async fn pastes_through_the_clipboard_and_restores_it() {
    let board = FakeBoard::holding("what the user copied");
    {
        let mut state = board.state.lock().unwrap();
        state.1[0].push(("public.rtf".to_owned(), b"{\\rtf1 what}".to_vec()));
        state
            .1
            .push(vec![("public.png".to_owned(), vec![0x89, 0x50])]);
    }
    let before = board.items();
    let (mut inserter, pasted) = inserter(&board);

    inserter.paste("Hello team.").await.unwrap();
    assert_eq!(*pasted.lock().unwrap(), [Some("Hello team.".to_owned())]);
    inserter.wait_for_restore();
    assert_eq!(board.items(), before);
}

/// AC-13: when the clipboard changes between the paste and the restore, the app does not
/// overwrite it.
#[tokio::test]
async fn a_clipboard_changed_after_the_paste_is_left_alone() {
    let board = FakeBoard::holding("old");
    let (mut inserter, _) = inserter(&board);
    inserter.paste("Dictated text.").await.unwrap();
    board.set("copied by the user meanwhile");
    inserter.wait_for_restore();
    assert_eq!(
        board.text().as_deref(),
        Some("copied by the user meanwhile")
    );
    assert!(!board.log.lock().unwrap().contains(&"restore".to_owned()));
}

/// R-16: a second paste before the first restore keeps the user's original clipboard, not the
/// first dictation, and restores it once.
#[tokio::test]
async fn back_to_back_pastes_restore_the_original() {
    let board = FakeBoard::holding("original");
    let (mut inserter, pasted) = inserter(&board);
    inserter.paste("First.").await.unwrap();
    inserter.paste("Second.").await.unwrap();
    inserter.wait_for_restore();
    std::thread::sleep(RESTORE_AFTER * 2);
    assert_eq!(
        *pasted.lock().unwrap(),
        [Some("First.".to_owned()), Some("Second.".to_owned())]
    );
    assert_eq!(board.text().as_deref(), Some("original"));
    let restores = board
        .log
        .lock()
        .unwrap()
        .iter()
        .filter(|l| *l == "restore")
        .count();
    assert_eq!(restores, 1);
}

/// An empty clipboard is restored as empty.
#[tokio::test]
async fn an_empty_clipboard_stays_empty() {
    let board = Arc::new(FakeBoard::default());
    let (mut inserter, _) = inserter(&board);
    inserter.paste("Text.").await.unwrap();
    inserter.wait_for_restore();
    assert!(board.items().is_empty());
}

/// R-31: a retry in another app copies; the text stays on the clipboard, and a restore still
/// pending from an earlier paste does not overwrite it.
#[tokio::test]
async fn copy_leaves_the_text_and_cancels_a_pending_restore() {
    let board = FakeBoard::holding("original");
    let (mut inserter, pasted) = inserter(&board);
    inserter.paste("Pasted first.").await.unwrap();
    inserter.copy("Copied after a retry.").await.unwrap();
    inserter.wait_for_restore();
    assert_eq!(board.text().as_deref(), Some("Copied after a retry."));
    assert_eq!(
        pasted.lock().unwrap().len(),
        1,
        "copy sends no paste shortcut"
    );
}
