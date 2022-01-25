extern crate graphite_proc_macros;

pub mod communication;
#[macro_use]
pub mod misc;
pub mod consts;
pub mod document;
pub mod frontend;
pub mod global;
pub mod input;
pub mod layout;
pub mod viewport_tools;

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
	pub use crate::communication::generate_uuid;
	pub use crate::communication::message::{AsMessage, Message, MessageDiscriminant};
	pub use crate::communication::message_handler::{ActionList, MessageHandler};

	pub use crate::document::clipboards::Clipboard;
	pub use crate::LayerId;

	pub use crate::layout::{LayoutMessage, LayoutMessageDiscriminant};
	pub use crate::document::{ArtboardMessage, ArtboardMessageDiscriminant};
	pub use crate::document::{DocumentMessage, DocumentMessageDiscriminant};
	pub use crate::document::{MovementMessage, MovementMessageDiscriminant};
	pub use crate::document::{OverlaysMessage, OverlaysMessageDiscriminant};
	pub use crate::document::{PortfolioMessage, PortfolioMessageDiscriminant};
	pub use crate::document::{TransformLayerMessage, TransformLayerMessageDiscriminant};
	pub use crate::frontend::{FrontendMessage, FrontendMessageDiscriminant};
	pub use crate::global::{GlobalMessage, GlobalMessageDiscriminant};
	pub use crate::input::{InputMapperMessage, InputMapperMessageDiscriminant, InputPreprocessorMessage, InputPreprocessorMessageDiscriminant};
	pub use crate::misc::derivable_custom_traits::{ToDiscriminant, TransitiveChild};
	pub use crate::viewport_tools::tool_message::{ToolMessage, ToolMessageDiscriminant};
	pub use crate::viewport_tools::tools::crop::{CropMessage, CropMessageDiscriminant};
	pub use crate::viewport_tools::tools::ellipse::{EllipseMessage, EllipseMessageDiscriminant};
	pub use crate::viewport_tools::tools::eyedropper::{EyedropperMessage, EyedropperMessageDiscriminant};
	pub use crate::viewport_tools::tools::fill::{FillMessage, FillMessageDiscriminant};
	pub use crate::viewport_tools::tools::line::{LineMessage, LineMessageDiscriminant};
	pub use crate::viewport_tools::tools::navigate::{NavigateMessage, NavigateMessageDiscriminant};
	pub use crate::viewport_tools::tools::path::{PathMessage, PathMessageDiscriminant};
	pub use crate::viewport_tools::tools::pen::{PenMessage, PenMessageDiscriminant};
	pub use crate::viewport_tools::tools::rectangle::{RectangleMessage, RectangleMessageDiscriminant};
	pub use crate::viewport_tools::tools::select::{SelectMessage, SelectMessageDiscriminant};
	pub use crate::viewport_tools::tools::shape::{ShapeMessage, ShapeMessageDiscriminant};
	pub use graphite_proc_macros::*;

	pub use std::collections::VecDeque;
}
