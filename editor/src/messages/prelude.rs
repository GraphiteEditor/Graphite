// Root
pub use crate::utility_traits::{ActionList, AsMessage, HierarchicalTree, MessageHandler, ToDiscriminant, TransitiveChild};
pub use crate::utility_types::{DebugMessageTree, MessageData};
// Message, MessageData, MessageDiscriminant, MessageHandler
pub use crate::messages::animation::{AnimationMessage, AnimationMessageDiscriminant, AnimationMessageHandler};
pub use crate::messages::app_window::{AppWindowMessage, AppWindowMessageDiscriminant, AppWindowMessageHandler};
pub use crate::messages::broadcast::{BroadcastMessage, BroadcastMessageDiscriminant, BroadcastMessageHandler};
pub use crate::messages::debug::{DebugMessage, DebugMessageDiscriminant, DebugMessageHandler};
pub use crate::messages::defer::{DeferMessage, DeferMessageDiscriminant, DeferMessageHandler};
pub use crate::messages::dialog::export_dialog::{ExportDialogMessage, ExportDialogMessageContext, ExportDialogMessageDiscriminant, ExportDialogMessageHandler};
pub use crate::messages::dialog::new_document_dialog::{NewDocumentDialogMessage, NewDocumentDialogMessageDiscriminant, NewDocumentDialogMessageHandler};
pub use crate::messages::dialog::preferences_dialog::{PreferencesDialogMessage, PreferencesDialogMessageContext, PreferencesDialogMessageDiscriminant, PreferencesDialogMessageHandler};
pub use crate::messages::dialog::{DialogMessage, DialogMessageContext, DialogMessageDiscriminant, DialogMessageHandler};
pub use crate::messages::frontend::{FrontendMessage, FrontendMessageDiscriminant};
pub use crate::messages::globals::{GlobalsMessage, GlobalsMessageDiscriminant, GlobalsMessageHandler};
pub use crate::messages::input_mapper::key_mapping::{KeyMappingMessage, KeyMappingMessageContext, KeyMappingMessageDiscriminant, KeyMappingMessageHandler};
pub use crate::messages::input_mapper::{InputMapperMessage, InputMapperMessageContext, InputMapperMessageDiscriminant, InputMapperMessageHandler};
pub use crate::messages::input_preprocessor::{InputPreprocessorMessage, InputPreprocessorMessageContext, InputPreprocessorMessageDiscriminant, InputPreprocessorMessageHandler};
pub use crate::messages::layout::{LayoutMessage, LayoutMessageDiscriminant, LayoutMessageHandler};
pub use crate::messages::portfolio::document::graph_operation::{GraphOperationMessage, GraphOperationMessageContext, GraphOperationMessageDiscriminant, GraphOperationMessageHandler};
pub use crate::messages::portfolio::document::navigation::{NavigationMessage, NavigationMessageContext, NavigationMessageDiscriminant, NavigationMessageHandler};
pub use crate::messages::portfolio::document::node_graph::{NodeGraphMessage, NodeGraphMessageDiscriminant, NodeGraphMessageHandler};
pub use crate::messages::portfolio::document::overlays::{OverlaysMessage, OverlaysMessageContext, OverlaysMessageDiscriminant, OverlaysMessageHandler};
pub use crate::messages::portfolio::document::properties_panel::{PropertiesPanelMessage, PropertiesPanelMessageDiscriminant, PropertiesPanelMessageHandler};
pub use crate::messages::portfolio::document::{DocumentMessage, DocumentMessageContext, DocumentMessageDiscriminant, DocumentMessageHandler};
pub use crate::messages::portfolio::menu_bar::{MenuBarMessage, MenuBarMessageDiscriminant, MenuBarMessageHandler};
pub use crate::messages::portfolio::spreadsheet::{SpreadsheetMessage, SpreadsheetMessageDiscriminant};
pub use crate::messages::portfolio::{PortfolioMessage, PortfolioMessageContext, PortfolioMessageDiscriminant, PortfolioMessageHandler};
pub use crate::messages::preferences::{PreferencesMessage, PreferencesMessageDiscriminant, PreferencesMessageHandler};
pub use crate::messages::tool::transform_layer::{TransformLayerMessage, TransformLayerMessageDiscriminant, TransformLayerMessageHandler};
pub use crate::messages::tool::{ToolMessage, ToolMessageContext, ToolMessageDiscriminant, ToolMessageHandler};
pub use crate::messages::workspace::{WorkspaceMessage, WorkspaceMessageDiscriminant, WorkspaceMessageHandler};

// Message, MessageDiscriminant
pub use crate::messages::broadcast::broadcast_event::{BroadcastEvent, BroadcastEventDiscriminant};
pub use crate::messages::message::{Message, MessageDiscriminant};
pub use crate::messages::tool::tool_messages::artboard_tool::{ArtboardToolMessage, ArtboardToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::brush_tool::{BrushToolMessage, BrushToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::eyedropper_tool::{EyedropperToolMessage, EyedropperToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::fill_tool::{FillToolMessage, FillToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::freehand_tool::{FreehandToolMessage, FreehandToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::gradient_tool::{GradientToolMessage, GradientToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::navigate_tool::{NavigateToolMessage, NavigateToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::path_tool::{PathToolMessage, PathToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::pen_tool::{PenToolMessage, PenToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::select_tool::{SelectToolMessage, SelectToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::shape_tool::{ShapeToolMessage, ShapeToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::spline_tool::{SplineToolMessage, SplineToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::text_tool::{TextToolMessage, TextToolMessageDiscriminant};

// Helper
pub use crate::messages::globals::global_variables::*;
pub use crate::messages::portfolio::document::utility_types::misc::DocumentId;
pub use graphite_proc_macros::*;
pub use std::collections::{HashMap, HashSet, VecDeque};

pub trait Responses {
	fn add(&mut self, message: impl Into<Message>);

	fn add_front(&mut self, message: impl Into<Message>);

	fn try_add(&mut self, message: Option<impl Into<Message>>) {
		if let Some(message) = message {
			self.add(message);
		}
	}
}

impl Responses for VecDeque<Message> {
	#[inline(always)]
	fn add(&mut self, message: impl Into<Message>) {
		self.push_back(message.into());
	}

	#[inline(always)]
	fn add_front(&mut self, message: impl Into<Message>) {
		self.push_front(message.into());
	}
}
