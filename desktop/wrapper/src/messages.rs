use graphite_editor::messages::prelude::FrontendMessage;
use std::path::PathBuf;

pub(crate) use graphite_editor::messages::prelude::Message as EditorMessage;

pub use graphite_editor::messages::frontend::utility_types::{DocumentInfo, PersistedState};
pub use graphite_editor::messages::input_mapper::utility_types::input_keyboard::{Key, ModifierKeys};
pub use graphite_editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState as MouseState, EditorPosition as Position, MouseKeys};
pub use graphite_editor::messages::prelude::DocumentId;
pub use graphite_editor::messages::prelude::InputPreprocessorMessage as InputMessage;
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
		document_serialized_content: String,
	},
	PersistenceDeleteDocument {
		id: DocumentId,
	},
	PersistenceWritePreferences {
		preferences: Preferences,
	},
	PersistenceLoadPreferences,
	PersistenceWriteState {
		state: PersistedState,
	},
	PersistenceReadState,
	PersistenceReadDocument {
		id: DocumentId,
	},
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
	Restart,
	LoadThirdPartyLicenses,
}

pub enum DesktopWrapperMessage {
	FromWeb(Box<EditorMessage>),
	Input(InputMessage),
	FileDialogResult { path: PathBuf, content: Vec<u8>, context: OpenFileDialogContext },
	SaveFileDialogResult { path: PathBuf, context: SaveFileDialogContext },
	OpenFile { path: PathBuf, content: Vec<u8> },
	ImportFile { path: PathBuf, content: Vec<u8> },
	PollNodeGraphEvaluation,
	UpdateMaximized { maximized: bool },
	UpdateFullscreen { fullscreen: bool },
	LoadDocumentContent { id: DocumentId, document: String },
	LoadPersistedState { state: PersistedState },
	LoadPreferences { preferences: Preferences },
	MenuEvent { id: String },
	ClipboardReadResult { content: Option<String> },
	PointerLockMove { x: f64, y: f64 },
	LoadThirdPartyLicenses { text: String },
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
