use crate::messages::prelude::*;
use graphite_proc_macros::*;

#[impl_message]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Message {
	// Sub-messages
	#[child]
	Animation(AnimationMessage),
	#[child]
	AppWindow(AppWindowMessage),
	#[child]
	Broadcast(BroadcastMessage),
	#[child]
	Clipboard(ClipboardMessage),
	#[child]
	Debug(DebugMessage),
	#[child]
	Defer(DeferMessage),
	#[child]
	Dialog(DialogMessage),
	#[child]
	Frontend(FrontendMessage),
	#[child]
	InputPreprocessor(InputPreprocessorMessage),
	#[child]
	KeyMapping(KeyMappingMessage),
	#[child]
	Layout(LayoutMessage),
	#[child]
	MenuBar(MenuBarMessage),
	#[child]
	Portfolio(PortfolioMessage),
	#[child]
	Preferences(PreferencesMessage),
	#[child]
	Tool(ToolMessage),
	#[child]
	Viewport(ViewportMessage),

	// Messages
	Batched {
		messages: Box<[Message]>,
	},
	NoOp,
}

/// Provides an impl of `specta::Type` for `MessageDiscriminant`, the struct created by `impl_message`.
/// Specta isn't integrated with `impl_message`, so a remote impl must be provided using this struct.
impl specta::Type for MessageDiscriminant {
	fn inline(_type_map: &mut specta::TypeCollection, _generics: specta::Generics) -> specta::DataType {
		specta::DataType::Any
	}
}

impl Message {
	pub fn message_tree() -> DebugMessageTree {
		Self::build_message_tree()
	}
}
