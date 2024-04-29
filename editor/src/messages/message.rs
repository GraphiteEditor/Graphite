use crate::messages::prelude::*;

use graphite_proc_macros::*;

#[impl_message]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Message {
	NoOp,
	Init,
	Batched(Box<[Message]>),

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
/// Specta isn't integrated with `impl_message`, so a remote impl must be provided using this struct.
impl specta::Type for MessageDiscriminant {
	fn inline(_type_map: &mut specta::TypeMap, _generics: specta::Generics) -> specta::DataType {
		specta::DataType::Any
	}
}
