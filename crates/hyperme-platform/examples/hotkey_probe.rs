//! Slice 7 live check: prints the hotkey events for 20 seconds. Hold Fn, let go, press Esc.
//! `cargo run -p hyperme-platform --example hotkey_probe` on a Mac; the terminal needs the
//! Input Monitoring permission. Prints event names only, never other keys.

fn main() {
    #[cfg(target_os = "macos")]
    {
        use hyperme_core::config::Hotkey;
        use hyperme_platform::hotkey::macos::HotkeyListener;

        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        let _listener = match HotkeyListener::start(&Hotkey::Fn, tx) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        };
        println!("Listening for 20 s: hold Fn, let go, press Esc.");
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(20) {
            if let Ok(event) = rx.try_recv() {
                println!("{:>6} ms  {event:?}", start.elapsed().as_millis());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS only for now");
}
