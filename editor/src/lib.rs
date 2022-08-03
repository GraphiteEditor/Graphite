extern crate graphite_proc_macros;

pub mod communication;
#[macro_use]
pub mod misc;
pub mod consts;
pub mod debug;
pub mod dialog;
pub mod document;
pub mod frontend;
pub mod input;
pub mod layout;
pub mod viewport_tools;
pub mod workspace;

#[doc(inline)]
pub use graphene::color::Color;
#[doc(inline)]
pub use graphene::document::Document as SvgDocument;
#[doc(inline)]
pub use graphene::LayerId;
#[doc(inline)]
pub use misc::EditorError;

use communication::dispatcher::Dispatcher;
use message_prelude::*;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	dispatcher: Dispatcher,
}

impl Editor {
	/// Construct a new editor instance.
	/// Remember to provide a random seed with `editor::communication::set_uuid_seed(seed)` before any editors can be used.
	pub fn new() -> Self {
		Self { dispatcher: Dispatcher::new() }
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Vec<FrontendMessage> {
		self.dispatcher.handle_message(message);

		let mut responses = Vec::new();
		std::mem::swap(&mut responses, &mut self.dispatcher.responses);

		responses
	}
}

impl Default for Editor {
	fn default() -> Self {
		Self::new()
	}
}

pub mod message_prelude {
	pub use crate::communication::broadcast_message::{BroadcastMessage, BroadcastMessageDiscriminant, BroadcastSignal, BroadcastSignalDiscriminant};
	pub use crate::communication::generate_uuid;
	pub use crate::communication::message::{AsMessage, Message, MessageDiscriminant};
	pub use crate::communication::message_handler::{ActionList, MessageHandler};

	pub use crate::document::clipboards::Clipboard;
	pub use crate::LayerId;

	pub use crate::debug::{DebugMessage, DebugMessageDiscriminant};
	pub use crate::dialog::messages::*;
	pub use crate::document::{ArtboardMessage, ArtboardMessageDiscriminant};
	pub use crate::document::{DocumentMessage, DocumentMessageDiscriminant};
	pub use crate::document::{MenuBarMessage, MenuBarMessageDiscriminant};
	pub use crate::document::{MovementMessage, MovementMessageDiscriminant};
	pub use crate::document::{OverlaysMessage, OverlaysMessageDiscriminant};
	pub use crate::document::{PortfolioMessage, PortfolioMessageDiscriminant};
	pub use crate::document::{PropertiesPanelMessage, PropertiesPanelMessageDiscriminant};
	pub use crate::document::{TransformLayerMessage, TransformLayerMessageDiscriminant};
	pub use crate::frontend::{FrontendMessage, FrontendMessageDiscriminant};
	pub use crate::input::{InputMapperMessage, InputMapperMessageDiscriminant, InputPreprocessorMessage, InputPreprocessorMessageDiscriminant};
	pub use crate::layout::{LayoutMessage, LayoutMessageDiscriminant};
	pub use crate::misc::derivable_custom_traits::{ToDiscriminant, TransitiveChild};
	pub use crate::viewport_tools::tool_message::{ToolMessage, ToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::artboard_tool::{ArtboardToolMessage, ArtboardToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::ellipse_tool::{EllipseToolMessage, EllipseToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::eyedropper_tool::{EyedropperToolMessage, EyedropperToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::fill_tool::{FillToolMessage, FillToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::freehand_tool::{FreehandToolMessage, FreehandToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::gradient_tool::{GradientToolMessage, GradientToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::line_tool::{LineToolMessage, LineToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::navigate_tool::{NavigateToolMessage, NavigateToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::path_tool::{PathToolMessage, PathToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::pen_tool::{PenToolMessage, PenToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::rectangle_tool::{RectangleToolMessage, RectangleToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::select_tool::{SelectToolMessage, SelectToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::shape_tool::{ShapeToolMessage, ShapeToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::spline_tool::{SplineToolMessage, SplineToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::text_tool::{TextToolMessage, TextToolMessageDiscriminant};
	pub use crate::workspace::{WorkspaceMessage, WorkspaceMessageDiscriminant};
	pub use graphite_proc_macros::*;

	pub use std::collections::VecDeque;
}
