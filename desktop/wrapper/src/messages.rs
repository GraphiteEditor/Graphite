use graphite_editor::messages::prelude::FrontendMessage;
use std::path::PathBuf;

pub(crate) use graphite_editor::messages::prelude::Message as EditorMessage;

pub use graphite_editor::messages::prelude::DocumentId;
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
	UpdateViewportInfo {
		x: f64,
		y: f64,
		width: f64,
		height: f64,
		scale: f64,
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
	UpdateMenu {
		entries: Vec<MenuItem>,
	},
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
	UpdateViewportInfo {
		x: f64,
		y: f64,
		width: f64,
		height: f64,
		scale: f64,
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
	MenuEvent {
		id: u64,
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

pub enum MenuItem {
	Action {
		id: u64,
		text: String,
		enabled: bool,
		shortcut: Option<Shortcut>,
	},
	Checkbox {
		id: u64,
		text: String,
		enabled: bool,
		shortcut: Option<Shortcut>,
		checked: bool,
	},
	SubMenu {
		id: u64,
		text: String,
		enabled: bool,
		items: Vec<MenuItem>,
	},
	Separator,
}

pub use keyboard_types::{Code as KeyCode, Modifiers};
pub struct Shortcut {
	pub key: KeyCode,
	pub modifiers: Modifiers,
}
