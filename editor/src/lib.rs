// since our policy is tabs, we want to stop clippy from warning about that
#![allow(clippy::tabs_in_doc_comments)]

extern crate graphite_proc_macros;

mod communication;
#[macro_use]
pub mod misc;
mod document;
mod frontend;
mod global;
pub mod input;
pub mod tool;

pub mod consts;

#[doc(inline)]
pub use misc::EditorError;

#[doc(inline)]
pub use graphene::color::Color;

#[doc(inline)]
pub use graphene::LayerId;

#[doc(inline)]
pub use graphene::document::Document as SvgDocument;

#[doc(inline)]
pub use frontend::Callback;

use communication::dispatcher::Dispatcher;
// TODO: serialize with serde to save the current editor state
pub struct Editor {
	dispatcher: Dispatcher,
}

use message_prelude::*;

impl Editor {
	pub fn new(callback: Callback) -> Self {
		Self {
			dispatcher: Dispatcher::new(callback),
		}
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Result<(), EditorError> {
		self.dispatcher.handle_message(message)
	}
}

pub mod message_prelude {
	pub use crate::communication::generate_uuid;
	pub use crate::communication::message::{AsMessage, Message, MessageDiscriminant};
	pub use crate::communication::{ActionList, MessageHandler};
	pub use crate::document::{DocumentMessage, DocumentMessageDiscriminant};
	pub use crate::document::{DocumentsMessage, DocumentsMessageDiscriminant};
	pub use crate::document::{MovementMessage, MovementMessageDiscriminant};
	pub use crate::document::{TransformLayerMessage, TransformLayerMessageDiscriminant};
	pub use crate::frontend::{FrontendMessage, FrontendMessageDiscriminant};
	pub use crate::global::{GlobalMessage, GlobalMessageDiscriminant};
	pub use crate::input::{InputMapperMessage, InputMapperMessageDiscriminant, InputPreprocessorMessage, InputPreprocessorMessageDiscriminant};
	pub use crate::misc::derivable_custom_traits::{ToDiscriminant, TransitiveChild};
	pub use crate::tool::tool_messages::*;
	pub use crate::tool::tools::crop::{CropMessage, CropMessageDiscriminant};
	pub use crate::tool::tools::eyedropper::{EyedropperMessage, EyedropperMessageDiscriminant};
	pub use crate::tool::tools::fill::{FillMessage, FillMessageDiscriminant};
	pub use crate::tool::tools::line::{LineMessage, LineMessageDiscriminant};
	pub use crate::tool::tools::navigate::{NavigateMessage, NavigateMessageDiscriminant};
	pub use crate::tool::tools::path::{PathMessage, PathMessageDiscriminant};
	pub use crate::tool::tools::pen::{PenMessage, PenMessageDiscriminant};
	pub use crate::tool::tools::rectangle::{RectangleMessage, RectangleMessageDiscriminant};
	pub use crate::tool::tools::select::{SelectMessage, SelectMessageDiscriminant};
	pub use crate::tool::tools::shape::{ShapeMessage, ShapeMessageDiscriminant};
	pub use crate::LayerId;
	pub use graphite_proc_macros::*;
	pub use std::collections::VecDeque;
}
