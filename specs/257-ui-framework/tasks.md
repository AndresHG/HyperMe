# Slices

Each slice is one pull request, checked on the owner's Mac (cargo fmt, clippy, tests, `npm run
check`, the screenshot checks) and measured where it touches a page.

1. **Tooling and the pill.** `app/web/` with Vite, Svelte 5, TypeScript, ESLint and svelte-check;
   `build.rs` builds it; unmigrated pages copied as they are; the pill as Svelte components; the
   screenshot checks served from the build; the token lint reads `.svelte` styles. AC-1, AC-2,
   AC-3, AC-6, AC-7.
2. **Bindings.** tauri-specta on every command and event; generated `bindings.ts`, checked in, and
   the staleness test; the pill uses them. AC-4. Done in PR 272: every page, not only the pill,
   uses them; the file is generated on a Mac, where the commands compile.
3. **Shared controls.** `lib/ui/`: Button, Switch, Field, Segmented, Card, Row, Menu, Sheet, from
   `components.css`. AC-3.
4. **Walkthrough.** onboarding as components. AC-6, AC-7.
5. **Home.** sidebar, Today, History (with its virtual list) and Dictionary. AC-6, AC-7.
6. **Settings in Home.** Settings as an on-demand component inside Home, no iframe. AC-5, AC-6.
   Includes the one redesign the owner approved (28 November 2026): an "AI models" group with
   Speech to text and Language model pages after `docs/ux/settings-models/` (mode rows, provider
   tabs, masked key with Edit, model list with an Active badge, fallback row), and an "Other
   model…" row for any model name with a Test button (`key_test` with that model). The model list
   is the provider's curated list plus, where the provider can list models, the live list
   (`models_list`). The spec gets an amendment for this redesign in that pull request.
   Split in two: 6a moves Settings as it is (PR 265); 6b builds the AI models pages (R-16 to
   R-20, AC-8).
7. **Clean-up.** `app/ui` removed, docs (DESIGN.md, CONTRIBUTING.md, README) updated. AC-1.
   `tokens.css` and `components.css` move to `app/web/src/lib/ui/` and are imported by each page;
   the build no longer copies a public folder.
