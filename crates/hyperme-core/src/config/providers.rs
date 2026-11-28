//! The provider catalogue: what each provider is (protocol, endpoint, key page, models and
//! prices), the built-in `default-providers.toml`, and the roles a provider can fill
//! (specs/96-provider-settings, R-1 to R-5).

use super::*;

/// The built-in provider catalogue (R-1). Entries in the user's file add to it or replace an
/// entry of the same name.
pub const DEFAULT_PROVIDERS: &str = include_str!("default-providers.toml");

/// How the app talks to a provider. The code knows protocols, not providers (R-2, D-7): adding
/// a provider that speaks one of these is a catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    /// Speechmatics' real-time WebSocket API.
    SpeechmaticsRt,
    /// An OpenAI-compatible `/chat/completions` API.
    OpenaiChat,
    /// OpenAI's Realtime API in a transcription session (slice 8).
    OpenaiRealtime,
}

impl Protocol {
    pub const ALL: [Protocol; 3] = [
        Protocol::SpeechmaticsRt,
        Protocol::OpenaiChat,
        Protocol::OpenaiRealtime,
    ];

    /// The name written in the catalogue.
    pub fn name(self) -> &'static str {
        match self {
            Protocol::SpeechmaticsRt => "speechmatics-rt",
            Protocol::OpenaiChat => "openai-chat",
            Protocol::OpenaiRealtime => "openai-realtime",
        }
    }

    /// The roles a client of this protocol can fill.
    pub fn roles(self) -> &'static [Role] {
        match self {
            Protocol::SpeechmaticsRt | Protocol::OpenaiRealtime => &[Role::Speech],
            Protocol::OpenaiChat => &[Role::Cleanup],
        }
    }

    /// The URL schemes its endpoint may use.
    pub(super) fn schemes(self) -> &'static [&'static str] {
        match self {
            Protocol::SpeechmaticsRt | Protocol::OpenaiRealtime => &["wss", "ws"],
            Protocol::OpenaiChat => &["https", "http"],
        }
    }
}

/// One job a model does (spec.md, "Requirements"). Answers and meeting cleanup join later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Role {
    Speech,
    Cleanup,
}

impl Role {
    /// The config section that names this role's provider.
    pub fn section(self) -> &'static str {
        match self {
            Role::Speech => "speech",
            Role::Cleanup => "cleanup",
        }
    }
}

pub(super) fn yes() -> bool {
    true
}

fn later_in_menus() -> u32 {
    500
}

/// Whether `endpoint` is a server on this Mac: its host is `127.0.0.1`, `localhost` or `[::1]`
/// (R-23).
pub fn on_this_mac(endpoint: &str) -> bool {
    let rest = endpoint.split_once("://").map_or(endpoint, |(_, r)| r);
    let authority = rest.split('/').next().unwrap_or("");
    let host = if authority.starts_with('[') {
        authority.split(']').next().map(|h| &h[1..]).unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// What a provider is (R-1).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEntry {
    /// As people write it: "Speechmatics", "Local server".
    pub name: String,
    pub protocol: Protocol,
    pub endpoint: String,
    /// Where "Get a key" goes.
    #[serde(default)]
    pub key_url: Option<String>,
    /// One line shown beside the key field while there is no key, such as a plan whose credits
    /// pay for it.
    #[serde(default)]
    pub key_note: Option<String>,
    /// The roles it is offered for; each must be one its protocol can fill.
    pub roles: Vec<Role>,
    /// False for a server that needs no key, such as one on this Mac (R-14).
    #[serde(default = "yes")]
    pub key_required: bool,
    /// Another catalogue entry whose key this one uses: one key per provider, whatever the
    /// protocol (D-2). OpenAI speech and OpenAI cleanup share the OpenAI key.
    #[serde(default)]
    pub key_from: Option<String>,
    /// The key can list the models it offers (R-15).
    #[serde(default)]
    pub list_models: bool,
    /// Listed models whose names contain any of these are left out of the menu: embeddings,
    /// speech and image models the role cannot use (R-15, design.md "Model listing").
    #[serde(default)]
    pub hide_models: Vec<String>,
    /// Credit new accounts get, for onboarding's line about it.
    #[serde(default)]
    pub free_credit_usd: Option<f64>,
    /// Place in a role's provider menu, lowest first, after the recommended one (R-7). Entries
    /// without one, such as a user's own, come after the built-in ones.
    #[serde(default = "later_in_menus")]
    pub order: u32,
    /// The models the catalogue knows, recommended first.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    /// Ports on this Mac where a server like it usually listens, tried in order when the user
    /// picks it (R-22).
    #[serde(default)]
    pub ports: Vec<u16>,
}

/// One model of a provider: its prices and the latency this project measured (R-1, R-19).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    /// Speech.
    #[serde(default)]
    pub price_per_hour_usd: Option<f64>,
    /// Cleanup.
    #[serde(default)]
    pub price_input_per_million_usd: Option<f64>,
    #[serde(default)]
    pub price_output_per_million_usd: Option<f64>,
    /// The day the prices were checked against the provider's page, "YYYY-MM-DD".
    #[serde(default)]
    pub prices_checked: Option<String>,
    /// The `reasoning_effort` values the model takes; empty means the field is never sent.
    #[serde(default)]
    pub reasoning_effort: Vec<String>,
    #[serde(default)]
    pub measured: Option<Measured>,
}

impl ModelEntry {
    /// A model the catalogue does not list, typed by hand (R-16): no prices, no measurement.
    pub fn named(id: &str) -> Self {
        ModelEntry {
            id: id.to_owned(),
            ..ModelEntry::default()
        }
    }
}

/// A latency this project measured, with the result it comes from (R-18, R-19).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measured {
    /// Median from key release to text.
    pub release_to_text_ms: u64,
    #[serde(default)]
    pub p90_ms: Option<u64>,
    /// The file under `docs/results/`.
    pub source: String,
}

/// The built-in catalogue file's shape.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalogue {
    providers: BTreeMap<String, ProviderEntry>,
}

/// The built-in catalogue, parsed and checked once.
pub(super) fn built_in_providers() -> &'static BTreeMap<String, ProviderEntry> {
    static PROVIDERS: std::sync::OnceLock<BTreeMap<String, ProviderEntry>> =
        std::sync::OnceLock::new();
    PROVIDERS.get_or_init(|| {
        let path = Path::new("default-providers.toml");
        let catalogue: Catalogue =
            toml::from_str(DEFAULT_PROVIDERS).expect("the built-in catalogue parses");
        for (id, entry) in &catalogue.providers {
            validate_provider(id, entry, path).expect("the built-in catalogue is valid");
        }
        catalogue.providers
    })
}
