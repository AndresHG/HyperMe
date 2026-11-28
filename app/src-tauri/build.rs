use std::path::Path;
use std::process::Command;

fn main() {
    build_pages();
    tauri_build::build();
}

/// Builds the pages with Vite into `app/web/dist`, which `tauri::generate_context!` embeds
/// (specs/257-ui-framework, R-2), so `cargo build`, `cargo test` and the release scripts need no
/// extra step. Runs again only when a page source changes. Node and npm must be installed.
fn build_pages() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("../web");
    for watched in [
        "src",
        "package.json",
        "package-lock.json",
        "vite.config.ts",
        "svelte.config.js",
        "tsconfig.json",
    ] {
        println!("cargo:rerun-if-changed={}", web.join(watched).display());
    }
    // Each page's entry, such as pill.html.
    for entry in std::fs::read_dir(&web).expect("app/web").flatten() {
        if entry.path().extension().is_some_and(|e| e == "html") {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
    println!("cargo:rerun-if-env-changed=HYPERME_SKIP_PAGES_BUILD");
    // For a machine that builds the pages itself first (and CI without Node): skip, if they exist.
    if std::env::var_os("HYPERME_SKIP_PAGES_BUILD").is_some() && web.join("dist").is_dir() {
        return;
    }
    if !web.join("node_modules").is_dir() {
        npm(&web, &["ci", "--no-audit", "--no-fund"]);
    }
    npm(&web, &["run", "build"]);
}

fn npm(dir: &Path, args: &[&str]) {
    let status = Command::new("npm")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| {
            panic!("building the pages needs Node.js and npm on the PATH (CONTRIBUTING.md): {e}")
        });
    assert!(
        status.success(),
        "`npm {}` in app/web failed: see the output above",
        args.join(" ")
    );
}
