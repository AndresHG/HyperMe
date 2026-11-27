//! The connector setup sheet as pure logic (specs/97-google-connectors, slice 6): the switches
//! a preset offers and the consent scopes they ask for (R-14, R-15), account labels and picking
//! an account by what the user said (R-11), the "In production" check (R-10a), and the Privacy
//! block (R-24). The network side is `hyperme_providers::setup`.

use serde::Serialize;

use crate::config::Connectors;
use crate::connector_settings::AccountEntry;
use crate::registry::{Connector, Registry};

/// The longest label: a word the user says aloud.
const MAX_LABEL: usize = 20;

/// Refresh tokens that end within this long mean a project left in "Testing" (R-10a). Google
/// gives such projects 7-day refresh tokens (design.md, "Token life").
const TESTING_REFRESH_LIMIT_S: u64 = 8 * 86_400;

/// One switch in the sheet's "What Hyper can see" step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Feature {
    /// A server id (`calendar`), or `server.feature` for an optional scope (`gmail.drafts`).
    pub id: String,
    pub label: String,
    pub on_by_default: bool,
}

/// The switches a connector offers: one per server with a label, then one per optional scope
/// with a label. All start on; G-2 has drafts on in the wizard.
pub fn features(connector: &Connector) -> Vec<Feature> {
    let mut features = Vec::new();
    for server in &connector.servers {
        if let Some(label) = &server.label {
            features.push(Feature {
                id: server.id.clone(),
                label: label.clone(),
                on_by_default: true,
            });
        }
        for (feature, label) in &server.optional_labels {
            features.push(Feature {
                id: format!("{}.{feature}", server.id),
                label: label.clone(),
                on_by_default: true,
            });
        }
    }
    features
}

/// The switches that are on for an account: its saved ones, or the defaults.
pub fn features_on(connector: &Connector, account: &AccountEntry) -> Vec<String> {
    if account.features.is_empty() {
        features(connector)
            .into_iter()
            .filter(|f| f.on_by_default)
            .map(|f| f.id)
            .collect()
    } else {
        account.features.clone()
    }
}

/// The scopes for a consent with `on` switched on (AC-7): each server's scopes, then its
/// optional ones, never one the preset forbids. An optional scope needs its server on.
pub fn consent_scopes(connector: &Connector, on: &[String]) -> Result<Vec<String>, String> {
    if on.is_empty() {
        return Err("switch on at least one thing Hyper may see".to_owned());
    }
    let offered = features(connector);
    let mut scopes: Vec<String> = Vec::new();
    let mut add = |list: &[String]| {
        for scope in list {
            if !scopes.contains(scope) {
                scopes.push(scope.clone());
            }
        }
    };
    for id in on {
        if !offered.iter().any(|f| &f.id == id) {
            return Err(format!("{} has no switch {id:?}", connector.name));
        }
        let (server_id, optional) = match id.split_once('.') {
            Some((server, feature)) => (server, Some(feature)),
            None => (id.as_str(), None),
        };
        let server = connector
            .server(server_id)
            .ok_or_else(|| format!("{} has no server {server_id:?}", connector.name))?;
        match optional {
            None => add(&server.scopes),
            Some(feature) => {
                if !on.iter().any(|other| other == server_id) {
                    return Err(format!(
                        "{:?} needs {:?} on too",
                        server
                            .optional_labels
                            .get(feature)
                            .map_or(id.as_str(), String::as_str),
                        server.label.as_deref().unwrap_or(server_id)
                    ));
                }
                add(server
                    .optional_scopes
                    .get(feature)
                    .map_or(&[], Vec::as_slice));
            }
        }
    }
    if let Some(bad) = scopes
        .iter()
        .find(|s| connector.forbidden_scopes.contains(s))
    {
        return Err(format!("{bad} is never asked for"));
    }
    Ok(scopes)
}

/// A label as stored: trimmed and lowercased, one short word (letters, digits, `-`, `_`).
fn label_of(label: &str) -> Result<String, String> {
    let label = label.trim().to_lowercase();
    let valid = !label.is_empty()
        && label.chars().count() <= MAX_LABEL
        && label.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && label
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if valid {
        Ok(label)
    } else {
        Err(format!(
            "a label is one word you can say, such as \"personal\" or \"work\", up to \
             {MAX_LABEL} letters"
        ))
    }
}

/// Adds the account after a sign-in, or replaces its switches when it signed in again.
pub fn add_account(
    connectors: &mut Connectors,
    connector: &str,
    label: &str,
    features: &[String],
) -> Result<String, String> {
    let label = label_of(label)?;
    if let Some(entry) = connectors
        .accounts
        .iter_mut()
        .find(|a| a.connector == connector && a.label == label)
    {
        entry.features = features.to_vec();
        entry.enabled = true;
        return Ok(entry.id());
    }
    let entry = AccountEntry {
        connector: connector.to_owned(),
        label,
        enabled: true,
        features: features.to_vec(),
    };
    let id = entry.id();
    connectors.accounts.push(entry);
    Ok(id)
}

/// Removes a disconnected account (`google:work`) from the config (R-12).
pub fn remove_account(connectors: &mut Connectors, account: &str) -> Result<(), String> {
    let before = connectors.accounts.len();
    connectors.accounts.retain(|a| a.id() != account);
    if connectors.accounts.len() == before {
        return Err(format!("{account} is not a connected account"));
    }
    Ok(())
}

/// The account a request means (AC-5): the one whose label it says ("in my work calendar"),
/// else the first account that is on. A label of an account that is off picks nothing.
pub fn account_for<'a>(
    text: &str,
    connector: &str,
    connectors: &'a Connectors,
) -> Option<&'a AccountEntry> {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect();
    let accounts = || {
        connectors
            .accounts
            .iter()
            .filter(|a| a.connector == connector)
    };
    if let Some(named) = accounts().find(|a| words.contains(&a.label)) {
        return named.enabled.then_some(named);
    }
    accounts().find(|a| a.enabled)
}

/// Whether a refresh token that ends in `refresh_expires_in` seconds comes from a project left
/// in "Testing" (R-10a). `None`: the token does not expire.
pub fn in_testing(refresh_expires_in: Option<u64>) -> bool {
    refresh_expires_in.is_some_and(|seconds| seconds <= TESTING_REFRESH_LIMIT_S)
}

/// One connected account in the Privacy block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrivacyAccount {
    pub title: String,
    /// What Hyper reads, in the sheet's words.
    pub reads: Vec<String>,
    /// That the service may ask before HyperMe reads, and who keeps the choice
    /// (specs/166-gmail-approvals, R-21).
    pub approvals: String,
}

/// Privacy → Connected accounts (R-24), in plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Privacy {
    pub accounts: Vec<PrivacyAccount>,
    pub where_it_goes: String,
    pub kept: String,
}

/// The Privacy block for the connected accounts.
pub fn privacy(registry: &Registry, connectors: &Connectors) -> Privacy {
    let accounts = connectors
        .accounts
        .iter()
        .filter_map(|account| {
            let connector = registry.get(&account.connector)?;
            let on = features_on(connector, account);
            let reads = features(connector)
                .into_iter()
                .filter(|f| on.contains(&f.id))
                .map(|f| f.label)
                .collect();
            Some(PrivacyAccount {
                title: format!("{} · {}", connector.name, account.label),
                reads,
                approvals: format!(
                    "{0} may ask for your OK before HyperMe reads some of it. {0} keeps your \
                     choice, not HyperMe.",
                    connector.name
                ),
            })
        })
        .collect();
    Privacy {
        accounts,
        // G-3, R-21.
        where_it_goes: "Calendar and mail content leaves this Mac only for your own language \
                        model provider, under your key, when a briefing, a summary, a sync or a \
                        question needs it."
            .to_owned(),
        // R-18, R-19.
        kept: "Events are kept from 1 day back to 7 days ahead and mail details (sender, \
               subject, snippet) for 14 days, then deleted. Mail bodies are never kept on disk."
            .to_owned(),
    }
}
