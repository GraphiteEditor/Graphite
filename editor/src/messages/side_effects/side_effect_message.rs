use std::time::Duration;

use crate::messages::prelude::*;

// Output messages are what the editor returns after processing Messages. It is handled by the scope outside the editor,
// which has access to the node graph executor, frontend, etc
#[impl_message(Message, SideEffect)]
#[derive(derivative::Derivative, Clone, serde::Serialize, serde::Deserialize)]
#[derivative(Debug, PartialEq)]
pub enum SideEffectMessage {
	// These messaged are automatically deduplicated and used to produce EditorOutputMessages
	// They are run at the end of the messages queue, and use the final editor state
	RenderNodeGraph,
	RefreshPropertiesPanel,
	DrawOverlays,
	RenderRulers,
	RenderScrollbars,
	UpdateLayerStructure,
	TriggerFontLoad,
	RequestDeferredMessage { message: Box<Message>, timeout: Duration },
}
