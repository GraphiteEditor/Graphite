mod color;
mod dispatcher;
mod error;
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
use tools::ToolState;
use workspace::Workspace;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	pub tools: ToolState,
	workspace: Workspace,
	dispatcher: Dispatcher,
}

impl Editor {
	pub fn new(callback: Callback) -> Self {
		Self {
			tools: ToolState::new(),
			workspace: Workspace::new(),
			dispatcher: Dispatcher::new(callback),
		}
	}
	pub fn handle_event(&mut self, event: events::Event) -> Result<(), EditorError> {
		self.dispatcher.handle_event(event)
	}
}
