use crate::messages::prelude::*;

use graphite_proc_macros::*;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[remain::sorted]
#[impl_message]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
	#[remain::unsorted]
	NoOp,
	#[remain::unsorted]
	Init,

	#[child]
	Broadcast(BroadcastMessage),
	#[child]
	Debug(DebugMessage),
	#[child]
	Dialog(DialogMessage),
	#[child]
	Frontend(FrontendMessage),
	#[child]
	Globals(GlobalsMessage),
	#[child]
	InputPreprocessor(InputPreprocessorMessage),
	#[child]
	KeyMapping(KeyMappingMessage),
	#[child]
	Layout(LayoutMessage),
	#[child]
	Portfolio(PortfolioMessage),
	#[child]
	Preferences(PreferencesMessage),
	#[child]
	Tool(ToolMessage),
	#[child]
	Workspace(WorkspaceMessage),
}

/// Provides an impl of `specta::Type` for `MessageDiscriminant`, the struct created by `impl_message`.
/// Specta isn't integrated with `impl_message`, so a remote impl must be provided using this
/// struct.
#[derive(specta::Type)]
#[specta(inline, remote = "MessageDiscriminant")]
pub struct MessageDiscriminantDef(u8);
