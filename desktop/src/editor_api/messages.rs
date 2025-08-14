use std::path::PathBuf;

use graphite_editor::messages::prelude::DocumentId;

pub enum NativeMessage {
	ToFrontend(Vec<u8>),
	OpenFileDialog {
		title: String,
		filters: Vec<FileFilter>,
		context: OpenFileDialogContext,
	},
	SaveFileDialog {
		title: String,
		default_filename: String,
		default_folder: Option<PathBuf>,
		content: Vec<u8>,
		context: SaveFileDialogContext,
	},
	OpenUrl(String),
	UpdateViewport(wgpu::Texture),
	UpdateViewportBounds {
		x: f32,
		y: f32,
		width: f32,
		height: f32,
	},
	UpdateOverlays(vello::Scene),
}

pub struct FileFilter {
	pub name: String,
	pub extensions: Vec<String>,
}

pub enum EditorMessage {
	FromFrontend(Vec<u8>),
	OpenFileDialogResult { path: PathBuf, content: Vec<u8>, context: OpenFileDialogContext },
	SaveFileDialogResult { path: PathBuf, context: SaveFileDialogContext },
}

pub enum OpenFileDialogContext {
	Document,
	Import,
}

pub enum SaveFileDialogContext {
	Document { document_id: DocumentId },
	Export,
}
