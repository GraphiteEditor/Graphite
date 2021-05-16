// since our policy is tabs, we want to stop clippy from warning about that
#![allow(clippy::tabs_in_doc_comments)]

#[macro_use]
mod macros;

mod communication;
mod document;
mod error;
pub mod hint;
pub mod tools;
pub mod workspace;

#[doc(inline)]
pub use error::EditorError;

#[doc(inline)]
pub use document_core::color::Color;

#[doc(inline)]
pub use document_core::LayerId;

#[doc(inline)]
pub use document_core::document::Document as SvgDocument;

#[doc(inline)]
pub use communication::events;

#[doc(inline)]
pub use communication::Callback;

use communication::dispatcher::Dispatcher;
// TODO: serialize with serde to save the current editor state
pub struct Editor {
	dispatcher: Dispatcher,
}

use communication::message::prelude::*;

impl Editor {
	pub fn new(callback: Callback) -> Self {
		Self {
			dispatcher: Dispatcher::new(callback),
		}
	}

	pub fn handle_event(&mut self, event: events::Event) -> Result<(), EditorError> {
		self.dispatcher.handle_message(InputPreprocessorMessage::Event(event).into())
	}
}
