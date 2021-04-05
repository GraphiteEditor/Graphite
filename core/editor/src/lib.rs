mod color;
mod dispatcher;
mod error;
#[macro_use]
mod macros;
pub mod tools;
pub mod workspace;

#[doc(inline)]
pub use error::EditorError;

#[doc(inline)]
pub use color::Color;

#[doc(inline)]
pub use dispatcher::events;

#[doc(inline)]
pub use dispatcher::Callback;

use dispatcher::Dispatcher;
use document_core::Document;
use tools::ToolState;
use workspace::Workspace;

pub struct EditorState {
	tool_state: ToolState,
	workspace: Workspace,
	document: Document,
}

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	state: EditorState,
	dispatcher: Dispatcher,
}

impl Editor {
	pub fn new(callback: Callback) -> Self {
		Self {
			state: EditorState {
				tool_state: ToolState::new(),
				workspace: Workspace::new(),
				document: Document::default(),
			},
			dispatcher: Dispatcher::new(callback),
		}
	}

	pub fn handle_event(&mut self, event: events::Event) -> Result<(), EditorError> {
		self.dispatcher.handle_event(&mut self.state, &event)
	}
}
