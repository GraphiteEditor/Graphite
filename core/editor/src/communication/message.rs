use std::fmt::Display;

use super::{document_action_handler::DocumentMessage, frontend::FrontendMessage, tool_action_handler::ToolMessage};

pub trait AsMessage: Sized + Into<Message> + Send + Sync + PartialEq<Message> + Display {
	fn name(&self) -> &str {}
	fn suffix(&self) -> &'static str {}
}

pub enum Message {
	Document(DocumentMessage),
	Tool(ToolMessage),
	Frontend(FrontendMessage),
}
pub enum MessageDiscriminant {
	Document(DocumentMessageDiscriminant),
	Tool(ToolMessageDiscriminant),
	Frontend(FrontendMessageDiscriminant),
}

/*SelectTool(ToolType),
SelectPrimaryColor(Color),
SelectSecondaryColor(Color),
SelectLayer(Vec<LayerId>),
SelectDocument(usize),
ToggleLayerVisibility(Vec<LayerId>),
ToggleLayerExpansion(Vec<LayerId>),
DeleteLayer(Vec<LayerId>),
AddFolder(Vec<LayerId>),
RenameLayer(Vec<LayerId>, String),
SwapColors,
ResetColors,
Undo,
Redo,
Center,
UnCenter,
Confirm,
SnapAngle,
UnSnapAngle,
LockAspectRatio,
UnlockAspectRatio,
Abort,
IncreaseSize,
DecreaseSize,
Save,
LogInfo,
LogDebug,
LogTrace,
// …
LmbDown,
RmbDown,
MmbDown,
LmbUp,
RmbUp,
MmbUp,
MouseMove,
TextKeyPress(char),*/
