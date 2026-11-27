//! Slice 4 of specs/97-google-connectors: connector presets as data, the user's custom servers,
//! tool defaults and the allowlist for jobs. Tests cite the acceptance criteria and
//! requirements in `specs/97-google-connectors/spec.md`.

use std::path::{Path, PathBuf};

use hyperme_core::config::Config;
use hyperme_core::registry::{Auth, Registry, ToolAccess, Transport};

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("registry")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const TEST_PRESET: &str = r#"
id = "test"
name = "Test"
kind = "oauth-mcp"
tools_verified = true
auth_url = "http://127.0.0.1:1/authorize"
token_url = "http://127.0.0.1:1/token"

[[servers]]
id = "notes"
url = "http://127.0.0.1:1/mcp"
scopes = ["notes.read"]

[tools]
read = ["list_notes"]
off = ["delete_*"]
"#;

fn error_of(dir: &Path) -> String {
    Registry::load(Some(dir), &[]).unwrap_err().to_string()
}

/// R-2, R-14: the Google preset ships as data, with two servers, its read scopes, drafts as an
/// optional scope, and never a scope that sends or changes mail. Its tool names are marked
/// unverified until slice 1 checks them.
#[test]
fn the_google_preset_is_built_in_data() {
    let registry = Registry::load(None, &[]).unwrap();
    let google = registry.get("google").unwrap();
    assert_eq!(google.name, "Google");
    assert!(google.builtin);
    // Checked against the live servers in slice 1 (27 November 2026).
    assert!(google.tools_verified);
    let ids: Vec<&str> = google.servers.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["calendar", "gmail"]);
    let Auth::OAuth(endpoints) = &google.auth else {
        panic!("google signs in with OAuth: {:?}", google.auth);
    };
    assert_eq!(endpoints.token_url, "https://oauth2.googleapis.com/token");
    assert!(
        endpoints
            .extra_auth_params
            .contains(&("access_type".to_owned(), "offline".to_owned()))
    );
    let gmail = google.server("gmail").unwrap();
    assert_eq!(
        gmail.transport,
        // /mcp/v1: Google's endpoint with form elicitation (specs/166, A-2).
        Transport::Http("https://gmailmcp.googleapis.com/mcp/v1".to_owned())
    );
    assert_eq!(
        gmail.optional_scopes["drafts"],
        ["https://www.googleapis.com/auth/gmail.compose"]
    );
    let all_scopes: Vec<&String> = google
        .servers
        .iter()
        .flat_map(|s| s.scopes.iter().chain(s.optional_scopes.values().flatten()))
        .collect();
    for banned in ["gmail.send", "gmail.modify", "https://mail.google.com/"] {
        assert!(
            all_scopes.iter().all(|s| !s.contains(banned)),
            "{banned} in {all_scopes:?}"
        );
    }
    assert_eq!(google.pulls[0].server, "calendar");
    assert_eq!(google.pulls[0].tool, "list_events");
}

/// R-5: reading tools default to read, creating tools to ask before, and trashing, spam and
/// label tools to off, through glob patterns. A tool no list names asks before, and the
/// strictest list wins.
#[test]
fn tool_defaults_follow_the_preset_with_globs() {
    let registry = Registry::load(None, &[]).unwrap();
    let google = registry.get("google").unwrap();
    assert_eq!(google.access("list_events"), ToolAccess::Read);
    assert_eq!(google.access("get_thread"), ToolAccess::Read);
    assert_eq!(google.access("create_draft"), ToolAccess::Ask);
    assert_eq!(google.access("trash_thread"), ToolAccess::Off);
    assert_eq!(google.access("mark_thread_spam"), ToolAccess::Off);
    assert_eq!(google.access("label_message"), ToolAccess::Off);
    assert_eq!(google.access("untrash_thread"), ToolAccess::Ask);
    assert_eq!(google.access("some_new_tool"), ToolAccess::Ask);

    let dir = temp_dir("strictest");
    std::fs::write(
        dir.join("both.toml"),
        TEST_PRESET.replace(r#"off = ["delete_*"]"#, r#"off = ["list_*"]"#),
    )
    .unwrap();
    let registry = Registry::load(Some(&dir), &[]).unwrap();
    assert_eq!(
        registry.get("test").unwrap().access("list_notes"),
        ToolAccess::Off
    );
}

/// AC-1: a preset added as a file in the presets folder is a connector like the built-in ones,
/// with no Rust change.
#[test]
fn a_preset_file_in_the_folder_adds_a_connector() {
    let dir = temp_dir("added");
    std::fs::write(dir.join("test.toml"), TEST_PRESET).unwrap();
    std::fs::write(dir.join("notes.txt"), "not a preset").unwrap();
    let registry = Registry::load(Some(&dir), &[]).unwrap();
    let ids: Vec<&str> = registry
        .connectors()
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(ids, ["google", "test"]);
    let test = registry.get("test").unwrap();
    assert!(!test.builtin);
    assert_eq!(
        test.server("notes").unwrap().transport,
        Transport::Http("http://127.0.0.1:1/mcp".to_owned())
    );
    assert_eq!(test.access("list_notes"), ToolAccess::Read);
    assert_eq!(test.access("delete_note"), ToolAccess::Off);
}

/// AC-10, R-22: a job's allowlist may hold read tools only; asking for a tool that asks
/// before or is off is refused, naming the tool.
#[test]
fn a_job_allowlist_takes_read_tools_only() {
    let registry = Registry::load(None, &[]).unwrap();
    let google = registry.get("google").unwrap();
    let allowed = google
        .job_allowlist(&["list_events".to_owned(), "search_threads".to_owned()])
        .unwrap();
    assert_eq!(allowed, ["list_events", "search_threads"]);
    for tool in ["create_draft", "trash_thread", "some_new_tool"] {
        let error = google
            .job_allowlist(&["list_events".to_owned(), tool.to_owned()])
            .unwrap_err();
        assert!(error.to_string().contains(tool), "{error}");
    }
}

/// R-2: a broken preset is refused with the file and the reason, never half loaded.
#[test]
fn broken_presets_are_refused_with_the_reason() {
    let cases = [
        (
            "unknown field",
            TEST_PRESET.replace("tools_verified", "tools_checked"),
            "tools_checked",
        ),
        (
            "duplicate server",
            format!(
                "{TEST_PRESET}\n[[servers]]\nid = \"notes\"\nurl = \"http://127.0.0.1:1/b\"\nscopes = []\n"
            ),
            "notes",
        ),
        (
            "plain http to a remote host",
            TEST_PRESET.replace("http://127.0.0.1:1/mcp", "http://example.com/mcp"),
            "https",
        ),
        (
            "bad id",
            TEST_PRESET.replace(r#"id = "test""#, r#"id = "Test Preset""#),
            "id",
        ),
        (
            "secret in the file",
            TEST_PRESET.replace("tools_verified = true", "client_secret = \"x\""),
            "client_secret",
        ),
        (
            "pull from an unknown server",
            format!(
                "{TEST_PRESET}\n[[pull]]\nserver = \"mail\"\ntool = \"list_notes\"\nkeep = []\ninto = \"notes\"\n"
            ),
            "mail",
        ),
        (
            "pull with a tool that is not read",
            format!(
                "{TEST_PRESET}\n[[pull]]\nserver = \"notes\"\ntool = \"delete_note\"\nkeep = []\ninto = \"notes\"\n"
            ),
            "delete_note",
        ),
    ];
    for (case, text, expected) in cases {
        let dir = temp_dir(&case.replace(' ', "_"));
        std::fs::write(dir.join("test.toml"), text).unwrap();
        let error = error_of(&dir);
        assert!(error.contains("test.toml"), "{case}: {error}");
        assert!(error.contains(expected), "{case}: {error}");
    }

    let dir = temp_dir("duplicate_id");
    std::fs::write(
        dir.join("google.toml"),
        TEST_PRESET.replace(r#"id = "test""#, r#"id = "google""#),
    )
    .unwrap();
    assert!(error_of(&dir).contains("google"));
}

const CUSTOM: &str = r#"
[[connectors.custom]]
id = "wiki"
name = "Team wiki"
url = "https://wiki.example.com/mcp"
auth = "bearer"

[[connectors.custom]]
id = "files"
name = "Local files"
command = "files-mcp"
args = ["--root", "/tmp"]
auth = "none"
tools = { read = ["read_*"], off = ["delete_*"] }

[[connectors.custom]]
id = "tracker"
name = "Tracker"
url = "https://tracker.example.com/mcp"
auth = "oauth"
auth_url = "https://tracker.example.com/authorize"
token_url = "https://tracker.example.com/token"
scopes = ["read"]
"#;

fn config_with(extra: &str) -> Result<Config, String> {
    let text = format!("{}\n{extra}", hyperme_core::config::DEFAULT_CONFIG);
    Config::parse(&text, Path::new("config.toml")).map_err(|e| e.to_string())
}

/// R-4: the user adds MCP servers by URL or by command, with OAuth, a bearer token (kept in the
/// keychain, never the file) or nothing. They join the registry with every tool asking before
/// unless the user's lists say otherwise.
#[test]
fn custom_servers_come_from_the_config() {
    let config = config_with(CUSTOM).unwrap();
    let registry = Registry::load(None, &config.connectors.custom).unwrap();
    let ids: Vec<&str> = registry
        .connectors()
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(ids, ["google", "wiki", "files", "tracker"]);

    let wiki = registry.get("wiki").unwrap();
    assert_eq!(wiki.auth, Auth::Bearer);
    assert_eq!(
        wiki.servers[0].transport,
        Transport::Http("https://wiki.example.com/mcp".to_owned())
    );
    assert_eq!(wiki.access("anything"), ToolAccess::Ask);

    let files = registry.get("files").unwrap();
    assert_eq!(files.auth, Auth::None);
    assert_eq!(
        files.servers[0].transport,
        Transport::Command {
            program: "files-mcp".to_owned(),
            args: vec!["--root".to_owned(), "/tmp".to_owned()],
        }
    );
    assert_eq!(files.access("read_file"), ToolAccess::Read);
    assert_eq!(files.access("delete_file"), ToolAccess::Off);

    let tracker = registry.get("tracker").unwrap();
    let Auth::OAuth(endpoints) = &tracker.auth else {
        panic!("{:?}", tracker.auth);
    };
    assert_eq!(endpoints.auth_url, "https://tracker.example.com/authorize");
    assert_eq!(tracker.servers[0].scopes, ["read"]);
}

/// R-4: a custom server needs a URL or a command (not both), OAuth needs its endpoints, ids
/// are unique across presets and custom servers, and a token never goes in the file. An older
/// config with no connectors section still loads.
#[test]
fn custom_servers_are_checked() {
    assert!(config_with("").unwrap().connectors.custom.is_empty());

    let both = "[[connectors.custom]]\nid = \"x\"\nname = \"X\"\nurl = \"https://x.example/mcp\"\ncommand = \"x\"\nauth = \"none\"\n";
    assert!(config_with(both).unwrap_err().contains("url or a command"));
    let neither = "[[connectors.custom]]\nid = \"x\"\nname = \"X\"\nauth = \"none\"\n";
    assert!(
        config_with(neither)
            .unwrap_err()
            .contains("url or a command")
    );
    let no_endpoints = "[[connectors.custom]]\nid = \"x\"\nname = \"X\"\nurl = \"https://x.example/mcp\"\nauth = \"oauth\"\n";
    assert!(
        config_with(no_endpoints)
            .unwrap_err()
            .contains("OAuth needs auth_url")
    );
    let token = "[[connectors.custom]]\nid = \"x\"\nname = \"X\"\nurl = \"https://x.example/mcp\"\nauth = \"bearer\"\nbearer_token = \"abc\"\n";
    let error = config_with(token).unwrap_err();
    assert!(error.contains("bearer_token"), "{error}");
    assert!(!error.contains("abc"), "{error}");

    let clash = "[[connectors.custom]]\nid = \"google\"\nname = \"Mine\"\nurl = \"https://x.example/mcp\"\nauth = \"none\"\n";
    let config = config_with(clash).unwrap();
    let error = Registry::load(None, &config.connectors.custom)
        .unwrap_err()
        .to_string();
    assert!(error.contains("google"), "{error}");
}
