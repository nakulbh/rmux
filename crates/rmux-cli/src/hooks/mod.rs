//! Agent hook installers and event handlers (Claude Code + OpenCode).
//!
//! Lifecycle:
//! 1. `rmux-cli hooks setup` writes agent config (settings.json / plugin)
//! 2. Agent fires a hook → shells out to `rmux-cli hooks <agent> <event>`
//! 3. Event handler classifies stdin JSON → `notification.create` + sidebar status
//!
//! When rmux is not running (no socket), handlers exit 0 so agents never hang.

mod events;
mod install;
mod registry;

pub use events::{ClaudeEvent, OpenCodeEvent, handle_claude_event, handle_opencode_event};
pub use install::{AgentChoice, InstallOutcome, InstallStatus, install_agents, uninstall_agents};
pub use registry::{AgentId, binary_on_path};
