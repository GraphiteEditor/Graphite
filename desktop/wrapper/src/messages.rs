use std::path::PathBuf;

use graphite_editor::messages::prelude::{DocumentId, FrontendMessage};

pub(crate) use graphite_editor::messages::prelude::Message as EditorMessage;

pub enum DesktopFrontendMessage {
	ToWeb(Vec<FrontendMessage>),
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
	UpdateViewportBounds {
		x: f32,
		y: f32,
		width: f32,
		height: f32,
	},
	UpdateOverlays(vello::Scene),
	UpdateWindowState {
		maximized: bool,
		minimized: bool,
	},
	CloseWindow,
}

pub struct FileFilter {
	pub name: String,
	pub extensions: Vec<String>,
}

pub enum DesktopWrapperMessage {
	FromWeb(Box<EditorMessage>),
	OpenFileDialogResult { path: PathBuf, content: Vec<u8>, context: OpenFileDialogContext },
	SaveFileDialogResult { path: PathBuf, context: SaveFileDialogContext },
	OpenDocument { path: PathBuf, content: Vec<u8> },
	OpenFile { path: PathBuf, content: Vec<u8> },
	ImportFile { path: PathBuf, content: Vec<u8> },
	ImportSvg { path: PathBuf, content: Vec<u8> },
	ImportImage { path: PathBuf, content: Vec<u8> },
	PollNodeGraphEvaluation,
	UpdatePlatform(Platform),
}

pub enum OpenFileDialogContext {
	Document,
	Import,
}

pub enum SaveFileDialogContext {
	Document { document_id: DocumentId, content: Vec<u8> },
	File { content: Vec<u8> },
}

pub enum Platform {
	Windows,
	Mac,
	Linux,
}
