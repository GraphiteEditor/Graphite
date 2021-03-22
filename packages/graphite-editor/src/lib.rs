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

// TODO: serialize with serde to save the current editor state
struct Editor {
	tools: ToolState,
	workspace: workspace::Workspace,
}
