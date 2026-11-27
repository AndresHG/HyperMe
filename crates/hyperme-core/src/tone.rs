//! Tone per app (issue 219): the cleanup's style follows the app the text goes into, from its
//! name alone, which is already inside the privacy boundary. Casual in chat, formal in mail,
//! lists and paragraphs in notes, and no rewriting in code editors and terminals.
//!
//! As in Wispr Flow's styles (docs.wisprflow.ai, "How to setup Flow Styles", read 27 November
//! 2026), a style changes capitals, punctuation and line breaks, never the words. The defaults
//! live here; a later version lets the user change them in Settings.

/// How text is shaped for the app in front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStyle {
    /// Slack, WhatsApp, Messages ...: casual, no full stop after a short message.
    Chat,
    /// Mail, Outlook ...: formal, greeting and sign-off on their own lines, paragraphs.
    Mail,
    /// Notes, Notion, Obsidian ...: lists and paragraphs.
    Notes,
    /// Code editors: hesitations out, nothing else changed, no formatting.
    Code,
    /// Terminals: as code, and never a line break, which a shell could run.
    Terminal,
    /// Any other app, or none known: the fixed prompt's rules only.
    Other,
}

/// App names as macOS shows them (`localizedName`), lower case.
const CHAT: &[&str] = &[
    "slack",
    "whatsapp",
    "messages",
    "telegram",
    "discord",
    "signal",
    "microsoft teams",
    "teams",
    "messenger",
    "beeper",
    "element",
    "zoom",
];
const MAIL: &[&str] = &[
    "mail",
    "microsoft outlook",
    "outlook",
    "spark",
    "spark desktop",
    "superhuman",
    "airmail",
    "mimestream",
    "thunderbird",
    "missive",
    "canary mail",
    "hey",
];
const NOTES: &[&str] = &[
    "notes",
    "notion",
    "obsidian",
    "bear",
    "craft",
    "pages",
    "microsoft word",
    "word",
    "textedit",
    "linear",
    "apple notes",
    "logseq",
    "things",
    "reminders",
];
const CODE: &[&str] = &[
    "code",
    "visual studio code",
    "visual studio code - insiders",
    "cursor",
    "windsurf",
    "zed",
    "xcode",
    "sublime text",
    "nova",
    "bbedit",
    "fleet",
    "android studio",
    "intellij idea",
    "pycharm",
    "webstorm",
    "rustrover",
    "goland",
    "clion",
    "rider",
    "phpstorm",
    "datagrip",
];
const TERMINAL: &[&str] = &[
    "terminal",
    "iterm",
    "iterm2",
    "warp",
    "ghostty",
    "alacritty",
    "kitty",
    "wezterm",
    "tabby",
    "hyper",
];

impl AppStyle {
    /// The style for the app named `app`, as the frontmost app reports it. Unknown is `Other`.
    pub fn for_app(app: Option<&str>) -> AppStyle {
        let Some(name) = app.map(|a| a.trim().to_lowercase()) else {
            return AppStyle::Other;
        };
        let name = name.as_str();
        let table: [(&[&str], AppStyle); 5] = [
            (TERMINAL, AppStyle::Terminal),
            (CODE, AppStyle::Code),
            (CHAT, AppStyle::Chat),
            (MAIL, AppStyle::Mail),
            (NOTES, AppStyle::Notes),
        ];
        table
            .iter()
            .find(|(names, _)| names.contains(&name))
            .map(|&(_, style)| style)
            // JetBrains apps add the edition: "IntelliJ IDEA Ultimate", "PyCharm CE".
            .or_else(|| {
                CODE.iter()
                    .any(|c| c.len() > 4 && name.starts_with(c))
                    .then_some(AppStyle::Code)
            })
            .unwrap_or(AppStyle::Other)
    }

    /// The last line of the cleanup prompt for this style and `app`: where the text goes and
    /// how to shape it. Always after the fixed prompt and the dictionary, so their start stays
    /// the same for every app and a provider's prompt cache keeps working.
    pub fn prompt_line(self, app: &str) -> String {
        let how = match self {
            AppStyle::Chat => {
                "a chat app. Keep the tone casual: capitals as usual, lighter punctuation, \
                 no full stop at the end of a short message, and no greeting or sign-off \
                 lines unless they were said."
            }
            AppStyle::Mail => {
                "an email. Keep the tone formal: full punctuation and capitals. When a \
                 greeting (Hi Ana, Hola Ana) or a sign-off (Thanks, Un saludo) was said, put \
                 it on its own line, with the name under the sign-off, and separate topics \
                 into paragraphs. Never add a greeting or sign-off that was not said."
            }
            AppStyle::Notes => "a notes app. Use lists and paragraphs where they help the reading.",
            AppStyle::Code => {
                "a code editor. Do not rewrite: remove only hesitations and false starts, \
                 keep code words, file names and commands exactly as said, and add no lists, \
                 line breaks or other formatting."
            }
            AppStyle::Terminal => {
                "a terminal. Do not rewrite: remove only hesitations and false starts, keep \
                 commands, flags and file names exactly as said, and output a single line \
                 with no lists or line breaks."
            }
            AppStyle::Other => return format!("\n\nThe text will be pasted into {app}."),
        };
        format!("\n\nThe text will be pasted into {app}, {how}")
    }

    /// Shapes the cleaned text after the model, where a rule is certain enough to need no
    /// model: list markers made plain (`- `), extra blank lines dropped, a terminal's text on
    /// one line, and a chat message's final full stop dropped when it is one short paragraph.
    pub fn finish(self, text: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        for line in text.trim().lines() {
            let line = line.trim_end();
            let indent = line.len() - line.trim_start().len();
            let body = line.trim_start();
            let body = ["* ", "• ", "· ", "– "]
                .iter()
                .find_map(|m| body.strip_prefix(m))
                .map_or_else(
                    || body.to_owned(),
                    |rest| format!("- {}", rest.trim_start()),
                );
            let body = body.replace("**", "");
            if body.is_empty() && lines.last().is_none_or(String::is_empty) {
                continue;
            }
            lines.push(format!("{}{body}", &line[..indent]));
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        match self {
            AppStyle::Terminal => lines
                .iter()
                .map(|l| l.trim().trim_start_matches("- "))
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            AppStyle::Chat if lines.len() == 1 => {
                let line = &lines[0];
                let sentences = line.matches(['.', '!', '?']).count();
                match line.strip_suffix('.') {
                    Some(rest) if sentences <= 2 && !rest.ends_with('.') => rest.to_owned(),
                    _ => line.clone(),
                }
            }
            _ => lines.join("\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue 219, AC-1: the app name picks the style; unknown apps and no app are `Other`.
    #[test]
    fn the_app_name_picks_the_style() {
        let style = |a| AppStyle::for_app(Some(a));
        assert_eq!(style("Slack"), AppStyle::Chat);
        assert_eq!(style("WhatsApp"), AppStyle::Chat);
        assert_eq!(style("Mail"), AppStyle::Mail);
        assert_eq!(style("Microsoft Outlook"), AppStyle::Mail);
        assert_eq!(style("Notes"), AppStyle::Notes);
        assert_eq!(style("Cursor"), AppStyle::Code);
        assert_eq!(style("Code"), AppStyle::Code);
        assert_eq!(style("PyCharm CE"), AppStyle::Code);
        assert_eq!(style("iTerm2"), AppStyle::Terminal);
        assert_eq!(style("Ghostty"), AppStyle::Terminal);
        assert_eq!(style("Safari"), AppStyle::Other);
        assert_eq!(style("Codeshot"), AppStyle::Other);
        assert_eq!(AppStyle::for_app(None), AppStyle::Other);
    }

    /// Issue 219, AC-2: an unknown app keeps today's line; a known one adds how to shape it.
    #[test]
    fn the_prompt_line_names_the_app_and_the_style() {
        assert_eq!(
            AppStyle::Other.prompt_line("Safari"),
            "\n\nThe text will be pasted into Safari."
        );
        let chat = AppStyle::Chat.prompt_line("Slack");
        assert!(chat.starts_with("\n\nThe text will be pasted into Slack, a chat app."));
        assert!(
            AppStyle::Terminal
                .prompt_line("Warp")
                .contains("single line")
        );
    }

    /// Issue 219, AC-3: a chat message loses its final full stop only when short.
    #[test]
    fn chat_drops_the_final_full_stop_of_a_short_message() {
        assert_eq!(AppStyle::Chat.finish("Sounds good."), "Sounds good");
        assert_eq!(
            AppStyle::Chat.finish("Yes. See you at 5."),
            "Yes. See you at 5"
        );
        assert_eq!(AppStyle::Chat.finish("Wait..."), "Wait...");
        assert_eq!(AppStyle::Chat.finish("Done? Sure!"), "Done? Sure!");
        let long = "One. Two. Three.";
        assert_eq!(AppStyle::Chat.finish(long), long);
        let list = "Need:\n- milk\n- eggs.";
        assert_eq!(AppStyle::Chat.finish(list), list);
        assert_eq!(AppStyle::Mail.finish("Thanks."), "Thanks.");
    }

    /// Issue 219, AC-4: a terminal never gets a line break, which a shell could run.
    #[test]
    fn a_terminal_gets_one_line() {
        assert_eq!(
            AppStyle::Terminal.finish("git status\n\n- then git diff"),
            "git status then git diff"
        );
    }

    /// Formatting, AC-5: list markers become `- `, bold and extra blank lines go.
    #[test]
    fn markers_are_made_plain_and_blank_lines_kept_single() {
        let text = "Three things:\n\n\n* **milk**\n• eggs\n  * bread\n\n";
        assert_eq!(
            AppStyle::Notes.finish(text),
            "Three things:\n\n- milk\n- eggs\n  - bread"
        );
        assert_eq!(AppStyle::Other.finish("1. one\n2. two"), "1. one\n2. two");
    }
}
