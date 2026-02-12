use graphite_editor::messages::prelude::FrontendMessage;
use std::path::PathBuf;

pub(crate) use graphite_editor::messages::prelude::Message as EditorMessage;

pub use graphite_editor::messages::input_mapper::utility_types::input_keyboard::{Key, ModifierKeys};
pub use graphite_editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState as MouseState, EditorPosition as Position, MouseKeys};
pub use graphite_editor::messages::prelude::InputPreprocessorMessage as InputMessage;

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
	UpdateViewportPhysicalBounds {
		x: f64,
		y: f64,
		width: f64,
		height: f64,
	},
	UpdateUIScale {
		scale: f64,
	},
	UpdateOverlays(vello::Scene),
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
	ClipboardRead,
	ClipboardWrite {
		content: String,
	},
	PointerLock,
	WindowClose,
	WindowMinimize,
	WindowMaximize,
	WindowFullscreen,
	WindowDrag,
	WindowHide,
	WindowHideOthers,
	WindowShowAll,
}

pub enum DesktopWrapperMessage {
	FromWeb(Box<EditorMessage>),
	Input(InputMessage),
	FileDialogResult {
		path: PathBuf,
		content: Vec<u8>,
		context: OpenFileDialogContext,
	},
	SaveFileDialogResult {
		path: PathBuf,
		context: SaveFileDialogContext,
	},
	OpenFile {
		path: PathBuf,
		content: Vec<u8>,
	},
	ImportFile {
		path: PathBuf,
		content: Vec<u8>,
	},
	PollNodeGraphEvaluation,
	UpdateMaximized {
		maximized: bool,
	},
	UpdateFullscreen {
		fullscreen: bool,
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
		id: String,
	},
	ClipboardReadResult {
		content: Option<String>,
	},
	PointerLockMove {
		x: f64,
		y: f64,
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
	Open,
	Import,
}

pub enum SaveFileDialogContext {
	Document { document_id: DocumentId, content: Vec<u8> },
	File { content: Vec<u8> },
}

pub enum MenuItem {
	Action {
		id: String,
		text: String,
		enabled: bool,
		shortcut: Option<Shortcut>,
	},
	Checkbox {
		id: String,
		text: String,
		enabled: bool,
		shortcut: Option<Shortcut>,
		checked: bool,
	},
	SubMenu {
		id: String,
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
