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
pub use document_core::color::Color;

#[doc(inline)]
pub use document_core::LayerId;

#[doc(inline)]
pub use document_core::document::Document as SvgDocument;

use communication::dispatcher::Dispatcher;
// TODO: serialize with serde to save the current editor state
pub struct Editor {
	dispatcher: Dispatcher,
}

use message_prelude::*;

impl Editor {
	pub fn new() -> Self {
		Self { dispatcher: Dispatcher::new() }
	}

	pub fn handle_message<T: Into<Message>>(&mut self, message: T) -> Result<Vec<FrontendMessage>, EditorError> {
		self.dispatcher.handle_message(message).map(|_| {
			let mut responses = Vec::new();
			std::mem::swap(&mut responses, &mut self.dispatcher.responses);
			responses
		})
	}
}

pub mod message_prelude {
	pub use super::communication::message::{AsMessage, Message, MessageDiscriminant};
	pub use super::communication::{ActionList, MessageHandler};
	pub use super::document::{DocumentMessage, DocumentMessageDiscriminant};
	pub use super::frontend::{FrontendMessage, FrontendMessageDiscriminant};
	pub use super::global::{GlobalMessage, GlobalMessageDiscriminant};
	pub use super::input::{InputMapperMessage, InputMapperMessageDiscriminant, InputPreprocessorMessage, InputPreprocessorMessageDiscriminant};
	pub use super::misc::derivable_custom_traits::{ToDiscriminant, TransitiveChild};
	pub use super::tool::tool_messages::*;
	pub use super::tool::tools::crop::{CropMessage, CropMessageDiscriminant};
	pub use super::tool::tools::eyedropper::{EyedropperMessage, EyedropperMessageDiscriminant};
	pub use super::tool::tools::line::{LineMessage, LineMessageDiscriminant};
	pub use super::tool::tools::navigate::{NavigateMessage, NavigateMessageDiscriminant};
	pub use super::tool::tools::path::{PathMessage, PathMessageDiscriminant};
	pub use super::tool::tools::pen::{PenMessage, PenMessageDiscriminant};
	pub use super::tool::tools::rectangle::{RectangleMessage, RectangleMessageDiscriminant};
	pub use super::tool::tools::select::{SelectMessage, SelectMessageDiscriminant};
	pub use super::tool::tools::shape::{ShapeMessage, ShapeMessageDiscriminant};
	pub use crate::LayerId;
	pub use graphite_proc_macros::*;
	pub use std::collections::VecDeque;
}
