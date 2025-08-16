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
		filters: Vec<FileFilter>,
		context: SaveFileDialogContext,
	},
	WriteFile {
		path: PathBuf,
		content: Vec<u8>,
	},
	OpenUrl(String),
	RequestRedraw,
	UpdateViewport(wgpu::Texture),
	UpdateViewportBounds {
		x: f32,
		y: f32,
		width: f32,
		height: f32,
	},
	UpdateOverlays(vello::Scene),
	Loopback(EditorMessage),
}

pub struct FileFilter {
	pub name: String,
	pub extensions: Vec<String>,
}

pub enum EditorMessage {
	FromFrontend(Vec<u8>),
	OpenFileDialogResult { path: PathBuf, content: Vec<u8>, context: OpenFileDialogContext },
	SaveFileDialogResult { path: PathBuf, context: SaveFileDialogContext },
	PoolNodeGraphEvaluation,
}

pub enum OpenFileDialogContext {
	Document,
	Import,
}

pub enum SaveFileDialogContext {
	Document { document_id: DocumentId, content: Vec<u8> },
	Export { content: Vec<u8> },
}
