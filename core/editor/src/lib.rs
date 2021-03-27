mod color;
mod error;
mod scheduler;
pub mod tools;
pub mod workspace;

#[doc(inline)]
pub use error::EditorError;

#[doc(inline)]
pub use color::Color;

use tools::ToolState;
use workspace::Workspace;

// TODO: serialize with serde to save the current editor state
pub struct Editor {
	pub tools: ToolState,
	workspace: Workspace,
}

impl Editor {
	pub fn new() -> Self {
		Self {
			tools: ToolState::new(),
			workspace: Workspace::new(),
		}
	}
}
