//! Running an action through its preview (specs/97-google-connectors, slice 9; R-23): a tool
//! that creates something runs only after the user presses Enter on the preview card; Esc
//! leaves nothing behind. The card is the app's; it answers through [`Confirmer`].

use std::fmt;

use hyperme_core::actions::{self, Gate, Preview};
use hyperme_core::connector_settings::AccountEntry;
use hyperme_core::registry::Connector;
use rmcp::model::CallToolResult;
use serde_json::Value;

use crate::mcp::{ConnectorError, RemoteServer};

/// What the user did on the preview card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Enter,
    Esc,
}

/// Shows the preview card and waits for Enter or Esc.
pub trait Confirmer {
    fn confirm(&self, preview: &Preview) -> impl Future<Output = Decision> + Send;
}

/// Why an action did not run.
#[derive(Debug)]
pub enum ActionError {
    /// Never runs, switched off, or needs a switch the account does not have.
    Refused(String),
    Connector(ConnectorError),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionError::Refused(reason) => write!(f, "{reason}"),
            ActionError::Connector(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ActionError {}

/// Runs `tool` with `args` on `server` for `account`: a read tool at once; an "ask before"
/// tool only after Enter on its preview, and nothing at all after Esc (`Ok(None)`).
pub async fn run_action<C: Confirmer + Sync>(
    server: &RemoteServer,
    connector: &Connector,
    account: &AccountEntry,
    tool: &str,
    args: Value,
    confirmer: &C,
) -> Result<Option<CallToolResult>, ActionError> {
    let gate = actions::gate(connector, account, tool).map_err(ActionError::Refused)?;
    if gate == Gate::Confirm {
        let preview = actions::preview(connector, account, tool, &args);
        if confirmer.confirm(&preview).await == Decision::Esc {
            return Ok(None);
        }
    }
    let arguments = match args {
        Value::Object(map) => Some(map),
        Value::Null => None,
        other => {
            return Err(ActionError::Refused(format!(
                "{tool}: arguments must be an object, not {other}"
            )));
        }
    };
    server
        .call_tool(tool, arguments, &[tool.to_owned()])
        .await
        .map(Some)
        .map_err(ActionError::Connector)
}
