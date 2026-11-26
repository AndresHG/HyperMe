//! The hint line under each role in Settings → Providers (specs/96-provider-settings, R-18 to
//! R-20): what the provider and model cost a month for the user's talking time, and how fast
//! they are. Prices and measurements come only from the catalogue, never a pricing API (R-19).

use crate::config::{Config, ModelEntry, Role};
use crate::cost::{cleanup_usd, shown_as, speech_usd};

/// One role's hint, in parts the page lays out on one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// "About $6 a month for 30 min a day", or "Price not in the catalogue".
    pub cost: String,
    /// "0.43 s from key release to text, measured", or "Speed not measured".
    pub speed: String,
    /// The result the measurement comes from, under `docs/results/`.
    pub source: Option<String>,
    /// "Your median 0.47 s over 50 dictations", with the timing log on (R-20).
    pub mine: Option<String>,
}

fn seconds(ms: u64) -> String {
    format!("{:.2} s", ms as f64 / 1000.0)
}

/// The hint for `role` using `provider` and `model`. `own` is the user's median in ms and the
/// number of dictations it covers, from the timing log.
pub fn hint(
    config: &Config,
    role: Role,
    provider: &str,
    model: &str,
    minutes_per_day: u32,
    own: Option<(u64, usize)>,
) -> Hint {
    let entry = config
        .provider(provider)
        .and_then(|p| p.models.iter().find(|m| m.id == model))
        .cloned()
        .unwrap_or_else(|| ModelEntry::named(model));
    let priced = match role {
        Role::Speech => entry.price_per_hour_usd.is_some(),
        Role::Cleanup => {
            entry.price_input_per_million_usd.is_some()
                && entry.price_output_per_million_usd.is_some()
        }
    };
    let cost = if priced {
        let usd = match role {
            Role::Speech => speech_usd(&entry, minutes_per_day),
            Role::Cleanup => cleanup_usd(&entry, &config.cleanup, minutes_per_day),
        };
        format!("{} for {minutes_per_day} min a day", shown_as(usd))
    } else {
        "Price not in the catalogue".to_owned()
    };
    let (speed, source) = match &entry.measured {
        Some(m) => (
            format!(
                "{} from key release to text, measured",
                seconds(m.release_to_text_ms)
            ),
            Some(m.source.clone()),
        ),
        None => ("Speed not measured".to_owned(), None),
    };
    Hint {
        cost,
        speed,
        source,
        mine: own.map(|(ms, n)| format!("Your median {} over {n} dictations", seconds(ms))),
    }
}
