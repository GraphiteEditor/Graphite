// since our policy is tabs, we want to stop clippy from warning about that
#![allow(clippy::tabs_in_doc_comments)]

#[macro_use]
mod macros;

mod dispatcher;
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
pub use dispatcher::events;

#[doc(inline)]
pub use dispatcher::Callback;

use dispatcher::Dispatcher;
use document::Document;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	dispatcher: Dispatcher,
}

impl Editor {
	pub fn new(callback: Callback) -> Self {
		Self {
			dispatcher: Dispatcher::new(callback),
		}
	}

	pub fn handle_event(&mut self, event: events::Event) -> Result<(), EditorError> {
		self.dispatcher.handle_event(event)
	}
}
