//! The model menu of a provider in Settings (specs/96-provider-settings, R-15 to R-17): the
//! models its key offers, the catalogue's first, or the catalogue's alone with a one-line notice
//! when the listing fails. Fetched when the menu opens, never on the dictation path.

use hyperme_core::config::{Config, Protocol};
use hyperme_core::secrets::SecretStore;

use crate::cleanup;

/// What a model menu shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelMenu {
    /// The catalogue's models first, then the key's own, without repeats.
    pub models: Vec<String>,
    /// One line when the listing failed (R-17).
    pub notice: Option<String>,
}

/// The menu for `provider`. Asks the provider for its list only when its catalogue entry says
/// it can list, its protocol is the chat protocol, and its key is there (or it needs none), so
/// choosing never sends anything to a provider without a key (R-30).
pub async fn model_menu(
    config: &Config,
    provider: &str,
    keys: &(dyn SecretStore + Sync),
) -> ModelMenu {
    let Some(entry) = config.provider(provider) else {
        return ModelMenu {
            models: Vec::new(),
            notice: None,
        };
    };
    let mut models: Vec<String> = entry.models.iter().map(|m| m.id.clone()).collect();
    let can_list = entry.list_models && entry.protocol == Protocol::OpenaiChat;
    let key = match keys.get(provider) {
        Ok(Some(key)) => Some(key),
        Ok(None) if !entry.key_required => Some(String::new()),
        _ => None,
    };
    let (true, Some(key)) = (can_list, key) else {
        return ModelMenu {
            models,
            notice: None,
        };
    };
    match cleanup::list_models(&entry.name, &entry.endpoint, &key).await {
        Ok(listed) => {
            let mut extra: Vec<String> = listed
                .into_iter()
                .filter(|id| {
                    let lower = id.to_lowercase();
                    !entry
                        .hide_models
                        .iter()
                        .any(|hidden| lower.contains(&hidden.to_lowercase()))
                })
                .filter(|id| !models.contains(id))
                .collect();
            extra.sort();
            extra.dedup();
            models.extend(extra);
            ModelMenu {
                models,
                notice: None,
            }
        }
        Err(error) => ModelMenu {
            models,
            notice: Some(format!(
                "Could not list {}'s models ({}), so these are the catalogue's.",
                entry.name,
                error.message.lines().next().unwrap_or_default()
            )),
        },
    }
}
