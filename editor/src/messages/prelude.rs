// Root
pub use crate::utility_traits::{ActionList, AsMessage, MessageHandler, ToDiscriminant, TransitiveChild};

// Message, MessageDiscriminant, MessageHandler
pub use crate::messages::broadcast::{BroadcastMessage, BroadcastMessageDiscriminant, BroadcastMessageHandler};
pub use crate::messages::debug::{DebugMessage, DebugMessageDiscriminant, DebugMessageHandler};
pub use crate::messages::dialog::export_dialog::{ExportDialogMessage, ExportDialogMessageDiscriminant, ExportDialogMessageHandler};
pub use crate::messages::dialog::new_document_dialog::{NewDocumentDialogMessage, NewDocumentDialogMessageDiscriminant, NewDocumentDialogMessageHandler};
pub use crate::messages::dialog::preferences_dialog::{PreferencesDialogMessage, PreferencesDialogMessageDiscriminant, PreferencesDialogMessageHandler};
pub use crate::messages::dialog::{DialogMessage, DialogMessageDiscriminant, DialogMessageHandler};
pub use crate::messages::frontend::{FrontendMessage, FrontendMessageDiscriminant};
pub use crate::messages::globals::{GlobalsMessage, GlobalsMessageDiscriminant, GlobalsMessageHandler};
pub use crate::messages::input_mapper::key_mapping::{KeyMappingMessage, KeyMappingMessageDiscriminant, KeyMappingMessageHandler};
pub use crate::messages::input_mapper::{InputMapperMessage, InputMapperMessageDiscriminant, InputMapperMessageHandler};
pub use crate::messages::input_preprocessor::{InputPreprocessorMessage, InputPreprocessorMessageDiscriminant, InputPreprocessorMessageHandler};
pub use crate::messages::layout::{LayoutMessage, LayoutMessageDiscriminant, LayoutMessageHandler};
pub use crate::messages::portfolio::document::navigation::{NavigationMessage, NavigationMessageDiscriminant, NavigationMessageHandler};
pub use crate::messages::portfolio::document::node_graph::{GraphOperationMessage, GraphOperationMessageDiscriminant, GraphOperationMessageHandler};
pub use crate::messages::portfolio::document::node_graph::{NodeGraphMessage, NodeGraphMessageDiscriminant, NodeGraphMessageHandler};
pub use crate::messages::portfolio::document::overlays::{OverlaysMessage, OverlaysMessageDiscriminant, OverlaysMessageHandler};
pub use crate::messages::portfolio::document::properties_panel::{PropertiesPanelMessage, PropertiesPanelMessageDiscriminant, PropertiesPanelMessageHandler};
pub use crate::messages::portfolio::document::{DocumentMessage, DocumentMessageDiscriminant, DocumentMessageHandler};
pub use crate::messages::portfolio::menu_bar::{MenuBarMessage, MenuBarMessageDiscriminant, MenuBarMessageHandler};
pub use crate::messages::portfolio::{PortfolioMessage, PortfolioMessageDiscriminant, PortfolioMessageHandler};
pub use crate::messages::preferences::{PreferencesMessage, PreferencesMessageDiscriminant, PreferencesMessageHandler};
pub use crate::messages::tool::transform_layer::{TransformLayerMessage, TransformLayerMessageDiscriminant, TransformLayerMessageHandler};
pub use crate::messages::tool::{ToolMessage, ToolMessageDiscriminant, ToolMessageHandler};
pub use crate::messages::workspace::{WorkspaceMessage, WorkspaceMessageDiscriminant, WorkspaceMessageHandler};

// Message, MessageDiscriminant
pub use crate::messages::broadcast::broadcast_event::{BroadcastEvent, BroadcastEventDiscriminant};
pub use crate::messages::message::{Message, MessageDiscriminant};
pub use crate::messages::tool::tool_messages::artboard_tool::{ArtboardToolMessage, ArtboardToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::brush_tool::{BrushToolMessage, BrushToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::ellipse_tool::{EllipseToolMessage, EllipseToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::eyedropper_tool::{EyedropperToolMessage, EyedropperToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::fill_tool::{FillToolMessage, FillToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::freehand_tool::{FreehandToolMessage, FreehandToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::gradient_tool::{GradientToolMessage, GradientToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::imaginate_tool::{ImaginateToolMessage, ImaginateToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::line_tool::{LineToolMessage, LineToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::navigate_tool::{NavigateToolMessage, NavigateToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::path_tool::{PathToolMessage, PathToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::pen_tool::{PenToolMessage, PenToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::polygon_tool::{PolygonToolMessage, PolygonToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::rectangle_tool::{RectangleToolMessage, RectangleToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::select_tool::{SelectToolMessage, SelectToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::spline_tool::{SplineToolMessage, SplineToolMessageDiscriminant};
pub use crate::messages::tool::tool_messages::text_tool::{TextToolMessage, TextToolMessageDiscriminant};

// Helper
pub use crate::messages::globals::global_variables::*;
pub use crate::messages::portfolio::document::node_graph::TransformIn;
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
	fn add(&mut self, message: impl Into<Message>) {
		self.push_back(message.into());
	}

	fn add_front(&mut self, message: impl Into<Message>) {
		self.push_front(message.into());
	}
}
