//! Slice 8 live check, read-only: the frontmost app, whether secure input is on, and how many
//! items and types the clipboard holds. Never prints clipboard contents and never pastes.

fn main() {
    #[cfg(target_os = "macos")]
    {
        use hyperme_core::traits::FocusInfo;
        use hyperme_platform::paste::Pasteboard;
        use hyperme_platform::paste::macos::{MacFocus, MacPasteboard};

        let focus = MacFocus;
        println!("frontmost app: {:?}", focus.frontmost_app());
        println!("secure input: {}", focus.secure_input());
        let items = MacPasteboard.save();
        let types: usize = items.iter().map(Vec::len).sum();
        println!(
            "clipboard: {} item(s), {types} type(s), change count {}",
            items.len(),
            MacPasteboard.change_count()
        );
    }
    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS only for now");
}
