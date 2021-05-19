// since our policy is tabs, we want to stop clippy from warning about that
#![allow(clippy::tabs_in_doc_comments)]

extern crate graphite_proc_macros;

mod communication;
#[macro_use]
mod misc;
mod document;
mod frontend;
mod global;
pub mod input;
pub mod tool;

#[doc(inline)]
pub use misc::EditorError;

#[doc(inline)]
pub use document_core::color::Color;

#[doc(inline)]
pub use document_core::LayerId;

#[doc(inline)]
pub use document_core::document::Document as SvgDocument;

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
	pub use super::communication::message::{AsMessage, Message, MessageDiscriminant};
	pub use super::communication::MessageHandler;
	pub use super::document::{DocumentMessage, DocumentMessageDiscriminant};
	pub use super::frontend::{FrontendMessage, FrontendMessageDiscriminant};
	pub use super::global::{GlobalMessage, GlobalMessageDiscriminant};
	pub use super::input::{InputMapperMessage, InputMapperMessageDiscriminant, InputPreprocessorMessage, InputPreprocessorMessageDiscriminant};
	pub use super::misc::derivable_custom_traits::{ToDiscriminant, TransitiveChild};
	pub use super::tool::tool_messages::*;
	pub use super::tool::tools::rectangle::{RectangleMessage, RectangleMessageDiscriminant};
	pub use graphite_proc_macros::*;
	pub use std::collections::VecDeque;
}
