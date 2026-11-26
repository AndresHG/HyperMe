//! Onboarding's cost estimate (docs/UX.md, "Models and keys in onboarding"; U-11): what the
//! recommended setup costs a month for a given amount of talking a day. Every price and estimate
//! comes from the config file, next to the model names (R-26); none is written here.

use crate::config::Config;

/// A month for the estimate.
const DAYS_PER_MONTH: f64 = 30.0;

/// One of the three choices the wizard offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub label: &'static str,
    pub minutes_per_day: u32,
    /// The same amount said another way, under the label.
    pub detail: &'static str,
}

/// U-11: three choices by talking time a day.
pub const CHOICES: [Choice; 3] = [
    Choice {
        label: "A little",
        minutes_per_day: 10,
        detail: "10 min a day, about 1 h a week",
    },
    Choice {
        label: "A fair amount",
        minutes_per_day: 30,
        detail: "30 min a day, about 3.5 h a week",
    },
    Choice {
        label: "A lot",
        minutes_per_day: 60,
        detail: "1 h a day, about 7 h a week",
    },
];

/// A month's cost in USD, by provider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    pub speech_usd: f64,
    pub cleanup_usd: f64,
}

impl Cost {
    pub fn total_usd(&self) -> f64 {
        self.speech_usd + self.cleanup_usd
    }
}

/// The monthly cost of `minutes_per_day` of talking (time actually speaking), a month being 30
/// days.
pub fn monthly_cost(config: &Config, minutes_per_day: u32) -> Cost {
    let hours = f64::from(minutes_per_day) * DAYS_PER_MONTH / 60.0;
    let c = &config.cleanup;
    let dictations = hours * c.estimate_dictations_per_hour();
    let per_token = |price_per_million: f64| price_per_million / 1_000_000.0;
    Cost {
        speech_usd: hours * config.speech.price_per_hour_usd(),
        cleanup_usd: dictations
            * (c.estimate_input_tokens() * per_token(c.price_input_per_million_usd())
                + c.estimate_output_tokens() * per_token(c.price_output_per_million_usd())),
    }
}

/// "About $6 a month", or cents under a dollar.
pub fn shown_as(usd: f64) -> String {
    if usd >= 1.0 {
        format!("About ${} a month", usd.round())
    } else {
        format!("About ${usd:.2} a month")
    }
}

/// The provider's name as people write it: "speechmatics" becomes "Speechmatics".
pub fn display_name(provider: &str) -> String {
    let mut chars = provider.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// "Speechmatics gives new accounts USD 100 of free credit, about 250 hours of talking.", or
/// nothing when the config names no credit.
pub fn free_credit_line(config: &Config) -> Option<String> {
    let credit = config.speech.free_credit_usd();
    let price = config.speech.price_per_hour_usd();
    if credit <= 0.0 || price <= 0.0 {
        return None;
    }
    Some(format!(
        "{} gives new accounts USD {} of free credit, about {} hours of talking.",
        display_name(&config.speech.provider),
        credit.round(),
        (credit / price).round()
    ))
}
