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

impl Message {
	pub fn message_tree() -> DebugMessageTree {
		Self::build_message_tree()
	}
}
