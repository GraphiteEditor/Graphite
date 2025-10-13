pub use graphite_editor::messages::prelude::DocumentId;
use graphite_editor::messages::prelude::FrontendMessage;
use std::path::PathBuf;

pub(crate) use graphite_editor::messages::prelude::Message as EditorMessage;

pub use graphite_editor::messages::prelude::PreferencesMessageHandler as Preferences;

pub enum DesktopFrontendMessage {
	ToWeb(Vec<FrontendMessage>),
	OpenLaunchDocuments,
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
	MinimizeWindow,
	MaximizeWindow,
	DragWindow,
	CloseWindow,
	PersistenceWriteDocument {
		id: DocumentId,
		document: Document,
	},
	PersistenceDeleteDocument {
		id: DocumentId,
	},
	PersistenceUpdateCurrentDocument {
		id: DocumentId,
	},
	PersistenceLoadCurrentDocument,
	PersistenceLoadRemainingDocuments,
	PersistenceUpdateDocumentsList {
		ids: Vec<DocumentId>,
	},
	PersistenceWritePreferences {
		preferences: Preferences,
	},
	PersistenceLoadPreferences,
}

pub enum DesktopWrapperMessage {
	FromWeb(Box<EditorMessage>),
	OpenFileDialogResult {
		path: PathBuf,
		content: Vec<u8>,
		context: OpenFileDialogContext,
	},
	SaveFileDialogResult {
		path: PathBuf,
		context: SaveFileDialogContext,
	},
	OpenDocument {
		path: PathBuf,
		content: Vec<u8>,
	},
	OpenFile {
		path: PathBuf,
		content: Vec<u8>,
	},
	ImportFile {
		path: PathBuf,
		content: Vec<u8>,
	},
	ImportSvg {
		path: PathBuf,
		content: Vec<u8>,
	},
	ImportImage {
		path: PathBuf,
		content: Vec<u8>,
	},
	PollNodeGraphEvaluation,
	UpdatePlatform(Platform),
	UpdateMaximized {
		maximized: bool,
	},
	LoadDocument {
		id: DocumentId,
		document: Document,
		to_front: bool,
		select_after_open: bool,
	},
	SelectDocument {
		id: DocumentId,
	},
	LoadPreferences {
		preferences: Option<Preferences>,
	},
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Document {
	pub content: String,
	pub name: String,
	pub path: Option<PathBuf>,
	pub is_saved: bool,
}

pub struct FileFilter {
	pub name: String,
	pub extensions: Vec<String>,
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
