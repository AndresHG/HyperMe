//! The typed bindings between Rust and the web pages (spec 257, R-8 and R-9): every command and
//! event the pages use, registered once here. `app/web/src/lib/bindings.ts` is generated from
//! this list and checked in; the test below fails when it is out of date, and
//! `HYPERME_UPDATE_BINDINGS=1 cargo test -p hyperme-app bindings` writes it again.

use tauri_specta::{Builder, collect_commands, collect_events};

/// Where the generated file lives, from this crate's folder.
#[cfg(test)]
const PATH: &str = "../web/src/lib/bindings.ts";

/// The commands and events, for `invoke_handler` and `mount_events`.
pub fn builder() -> Builder<tauri::Wry> {
    events::<tauri::Wry>().commands(collect_commands![
        crate::runtime::pill_clicked,
        crate::runtime::pill_hands_free,
        crate::home::home_sections,
        crate::home::home_blocks,
        crate::home::home_attention,
        crate::home::home_refresh,
        crate::home::home_stats,
        crate::home::home_page,
        crate::home::home_search,
        crate::home::home_info,
        crate::home::home_failed,
        crate::home::home_item,
        crate::home::home_copy,
        crate::home::home_paste,
        crate::home::home_retry,
        crate::home::home_delete,
        crate::home::home_fix_word,
        crate::samples::home_correct,
        crate::samples::home_delete_sample,
        crate::samples::samples_view,
        crate::samples::samples_delete_all,
        crate::samples::samples_reveal,
        crate::runtime::pill_view,
        crate::diagnostics::diagnostics_view,
        crate::diagnostics::history_trace,
        crate::report::report_build,
        crate::report::report_save,
        crate::report::report_open_issue,
        crate::runtime::pill_card,
        crate::runtime::card_open_today,
        crate::runtime::card_dismiss,
        crate::runtime::card_action,
        crate::settings::settings_view,
        crate::settings::settings_set,
        crate::settings::microphones_list,
        crate::settings::microphones_set,
        crate::settings::key_set,
        crate::settings::key_test,
        crate::settings::provider_choose,
        crate::settings::fallback_set,
        crate::settings::fallback_clear,
        crate::settings::providers_reset,
        crate::settings::provider_add,
        crate::settings::provider_key_page,
        crate::settings::models_list,
        crate::settings::local_discover,
        crate::settings::local_use,
        crate::settings::provider_hint,
        crate::settings::result_open,
        crate::settings::settings_open,
        crate::settings::settings_pending,
        crate::settings::settings_opened,
        crate::settings::settings_closed,
        crate::settings::app_restart,
        crate::dictionary::dictionary_view,
        crate::dictionary::dictionary_save,
        crate::dictionary::dictionary_import,
        crate::dictionary::dictionary_export,
        crate::dictionary::dictionary_is_common,
        crate::dictionary::dictionary_suggestion,
        crate::dictionary::dictionary_learning,
        crate::dictionary::dictionary_suggested,
        crate::connectors::connectors_view,
        crate::connectors::connectors_set_tool,
        crate::connectors::connectors_set_enabled,
        crate::connectors::connectors_add_custom,
        crate::connectors::connectors_set_token,
        crate::connectors::connectors_open_link,
        crate::connectors::connectors_save_client,
        crate::connectors::connectors_sign_in,
        crate::connectors::connectors_cancel_sign_in,
        crate::connectors::connectors_disconnect,
        crate::connectors::connectors_review,
        crate::approvals::connectors_answer,
        crate::connectors::connectors_open_url,
        crate::tasks::home_today,
        crate::tasks::tasks_add,
        crate::tasks::tasks_act,
        crate::tasks::tasks_open_meeting,
        crate::walkthrough::onboarding_checks,
        crate::walkthrough::onboarding_mic,
        crate::walkthrough::onboarding_request,
        crate::walkthrough::onboarding_try,
        crate::walkthrough::onboarding_can_continue,
        crate::walkthrough::onboarding_keys_screen,
        crate::walkthrough::onboarding_get_key,
        crate::walkthrough::onboarding_other_providers,
        crate::walkthrough::onboarding_relaunch,
        crate::walkthrough::onboarding_done,
    ])
}

/// The events alone, for any runtime: an event can only be sent once they are mounted, in the
/// app and in tests with Tauri's mock runtime.
pub fn events<R: tauri::Runtime>() -> Builder<R> {
    Builder::<R>::new()
        .events(collect_events![
            crate::events::Pill,
            crate::events::PillHidden,
            crate::events::Card,
            crate::events::Level,
            crate::events::OnboardingLevel,
            crate::events::DictationPasted,
            crate::events::ProvidersChanged,
            crate::events::HomeAttention,
            crate::events::HomeRefresh,
            crate::events::HomeChanged,
            crate::events::HomeRetry,
            crate::events::HomeBlockViewed,
            crate::events::ConnectorApproval,
            crate::events::ConnectorApprovalClosed,
        ])
        // Ids, sizes and milliseconds stay far below 2^53, so they are plain numbers in
        // TypeScript rather than bigints.
        .dangerously_cast_bigints_to_number()
}

/// The TypeScript file as it should be.
#[cfg(test)]
fn render() -> String {
    let dir = std::env::temp_dir().join(format!("hyperme-bindings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary folder");
    let file = dir.join("bindings.ts");
    builder()
        .export(specta_typescript::Typescript::default().header("// Generated from the Rust commands by tauri-specta (app/src-tauri/src/bindings.rs). Do not edit.\n/* eslint-disable */\n// @ts-nocheck\n"), &file)
        .expect("bindings export");
    let text = std::fs::read_to_string(&file).expect("bindings read");
    let _ = std::fs::remove_dir_all(&dir);
    text
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn sources(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("a source folder") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                sources(&path, extensions, out);
            } else if path
                .extension()
                .is_some_and(|e| extensions.iter().any(|x| e == *x))
            {
                out.push(path);
            }
        }
    }

    /// AC-4 (R-8): no page calls a command or listens to an event by a name in a string; only
    /// the generated bindings talk to Tauri's API.
    #[test]
    fn only_the_bindings_call_tauri() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/src");
        let mut files = Vec::new();
        sources(&root, &["ts", "svelte"], &mut files);
        assert!(files.len() > 20, "pages found");
        for path in files {
            if path.ends_with("lib/bindings.ts") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source");
            for banned in [
                "invoke(",
                "invoke<",
                "@tauri-apps/api/core",
                "listen<",
                "listen('",
                "emit('",
            ] {
                assert!(
                    !text.contains(banned),
                    "{} uses {banned}: go through lib/ipc.ts and the generated bindings",
                    path.display()
                );
            }
        }
    }

    /// A plain `#[tauri::command]` runs on the main thread, where a slow database, keychain or
    /// file call freezes every window. Only the commands below stay there: they touch windows,
    /// in-memory state or the walkthrough, and return at once. Every other command is
    /// `#[tauri::command(async)]` or an `async fn`, so it runs off the main thread.
    #[test]
    fn only_quick_commands_run_on_the_main_thread() {
        const MAIN_THREAD: &[&str] = &[
            "app_restart",
            "card_action",
            "card_dismiss",
            "card_open_today",
            "connectors_answer",
            "connectors_cancel_sign_in",
            "connectors_open_link",
            "connectors_open_url",
            "dictionary_is_common",
            "home_attention",
            "home_blocks",
            "home_refresh",
            "home_sections",
            "onboarding_can_continue",
            "onboarding_checks",
            "onboarding_done",
            "onboarding_get_key",
            "onboarding_keys_screen",
            "onboarding_mic",
            "onboarding_other_providers",
            "onboarding_relaunch",
            "onboarding_request",
            "onboarding_try",
            "pill_card",
            "pill_clicked",
            "pill_hands_free",
            "pill_view",
            "provider_key_page",
            "result_open",
            "settings_closed",
            "settings_open",
            "settings_opened",
            "settings_pending",
            "tasks_open_meeting",
        ];
        let mut files = Vec::new();
        sources(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &["rs"],
            &mut files,
        );
        let mut on_main = Vec::new();
        for path in files {
            let text = std::fs::read_to_string(&path).expect("source");
            let mut lines = text.lines();
            while let Some(line) = lines.next() {
                if line.trim() != "#[tauri::command]" {
                    continue;
                }
                let signature = lines
                    .by_ref()
                    .find(|l| l.contains("fn "))
                    .expect("a command's fn");
                if signature.contains("async fn") {
                    continue;
                }
                let name = signature.split("fn ").nth(1).expect("name");
                on_main.push(name[..name.find('(').expect("(")].to_string());
            }
        }
        on_main.sort();
        assert_eq!(
            on_main, MAIN_THREAD,
            "make the new command #[tauri::command(async)]"
        );
    }

    /// AC-4: the checked-in bindings match the Rust commands, so a renamed argument fails here
    /// until the file is generated again.
    #[test]
    fn bindings_are_current() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(super::PATH);
        let want = super::render();
        if std::env::var_os("HYPERME_UPDATE_BINDINGS").is_some() {
            std::fs::write(&path, &want).expect("bindings write");
            return;
        }
        let have = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            have == want,
            "app/web/src/lib/bindings.ts is out of date: run HYPERME_UPDATE_BINDINGS=1 cargo test -p hyperme-app bindings"
        );
    }
}
