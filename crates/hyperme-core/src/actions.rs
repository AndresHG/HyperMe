//! Actions that create something outside the app (specs/97-google-connectors, slice 9):
//! whether a tool runs, asks first or never runs, and the preview the card shows before Enter
//! (R-5, R-23). Email is draft-only (`CLAUDE.md`). The card itself is the phase 3 preview
//! card; this is what it shows and what it guards.

use serde_json::Value;

use crate::connector_settings::AccountEntry;
use crate::connector_setup::{features, features_on};
use crate::registry::{Connector, ToolAccess, glob};

/// What happens when a tool is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// A read tool: it runs.
    Run,
    /// Show the preview and wait for Enter (R-23).
    Confirm,
}

/// Whether `tool` may run for `account`, and how. Refused: a tool the preset never runs (such
/// as one that sends mail), a tool switched off, and a tool whose setup switch is off for the
/// account, which needs a new consent (R-15).
pub fn gate(connector: &Connector, account: &AccountEntry, tool: &str) -> Result<Gate, String> {
    if connector.forbidden_tools.iter().any(|p| glob(p, tool)) {
        return Err(format!("{tool} never runs: email is draft-only"));
    }
    let gate = match connector.access(tool) {
        ToolAccess::Read => Gate::Run,
        ToolAccess::Ask => Gate::Confirm,
        ToolAccess::Off => return Err(format!("{tool} is switched off")),
    };
    let action = connector.actions.iter().find(|a| a.tool == tool);
    if let Some(needs) = action.and_then(|a| a.needs.as_ref())
        && !features_on(connector, account).contains(needs)
    {
        let label = features(connector)
            .into_iter()
            .find(|f| &f.id == needs)
            .map_or(needs.clone(), |f| f.label);
        return Err(format!(
            "switch on \"{label}\" for {} · {} and sign in again",
            connector.name, account.label
        ));
    }
    Ok(gate)
}

/// What the preview card shows (R-23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub title: String,
    /// Each argument shown, with its label.
    pub lines: Vec<(String, String)>,
    /// The Enter button's words.
    pub enter: String,
}

fn shown(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Array(list) if !list.is_empty() => Some(
            list.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The preview of `tool` with `args`: the preset's title, labels and order, or every argument
/// when the preset has no preview for it.
pub fn preview(connector: &Connector, account: &AccountEntry, tool: &str, args: &Value) -> Preview {
    let place = format!("{} · {}", connector.name, account.label);
    match connector.actions.iter().find(|a| a.tool == tool) {
        Some(action) => Preview {
            title: format!("{} in {place}", action.title),
            lines: action
                .fields
                .iter()
                .filter_map(|[key, label]| Some((label.clone(), shown(args.get(key)?)?)))
                .collect(),
            enter: action.enter.clone(),
        },
        None => Preview {
            title: format!("Run {tool} in {place}"),
            lines: args
                .as_object()
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| Some((k.clone(), shown(v)?)))
                        .collect()
                })
                .unwrap_or_default(),
            enter: "Run".to_owned(),
        },
    }
}
