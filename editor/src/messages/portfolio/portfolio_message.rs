use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::utility_types::PanelType;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::prelude::*;
use crate::node_graph_executor::IntrospectionResponse;
use graph_craft::document::CompilationMetadata;
use graphene_std::Color;
use graphene_std::raster::Image;
use graphene_std::renderer::RenderMetadata;
use graphene_std::text::Font;
use graphene_std::uuid::CompiledProtonodeInput;

#[impl_message(Message, Portfolio)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PortfolioMessage {
	// Sub-messages
	#[child]
	MenuBar(MenuBarMessage),
	#[child]
	Document(DocumentMessage),
	#[child]
	Spreadsheet(SpreadsheetMessage),

	// Introspected data is cleared after all queued messages which relied on the introspection are complete
	ClearIntrospectedData,

	// Sends a request to compile the network. Should occur when any value, preference, or font changes
	CompileActiveDocument,
	// Sends a request to evaluate the network. Should occur when any context value changes.
	EvaluateActiveDocument,
	// Sends a request to introspect data in the network, and return it to the editor
	IntrospectActiveDocument {
		inputs_to_introspect: HashSet<CompiledProtonodeInput>,
	},
	ExportActiveDocument {
		file_name: String,
		file_type: FileType,
		scale_factor: f64,
		bounds: ExportBounds,
		transparent_background: bool,
	},
	// Processes the compilation response and updates the data stored in the network interface for the active document
	// TODO: Add document ID in response for stability
	ProcessCompilationResponse {
		compilation_metadata: CompilationMetadata,
	},
	ProcessEvaluationResponse {
		evaluation_metadata: RenderMetadata,
	},
	ProcessIntrospectionResponse {
		#[serde(skip)]
		introspected_inputs: IntrospectionResponse,
	},
	RenderThumbnails,
	ProcessThumbnails,
	DocumentPassMessage {
		document_id: DocumentId,
		message: DocumentMessage,
	},
	AutoSaveActiveDocument,
	AutoSaveAllDocuments,
	AutoSaveDocument {
		document_id: DocumentId,
	},
	CloseActiveDocumentWithConfirmation,
	CloseAllDocuments,
	CloseAllDocumentsWithConfirmation,
	CloseDocument {
		document_id: DocumentId,
	},
	CloseDocumentWithConfirmation {
		document_id: DocumentId,
	},
	Copy {
		clipboard: Clipboard,
	},
	Cut {
		clipboard: Clipboard,
	},
	DeleteDocument {
		document_id: DocumentId,
	},
	DestroyAllDocuments,
	FontLoaded {
		font_family: String,
		font_style: String,
		preview_url: String,
		data: Vec<u8>,
	},
	Import,
	LoadDocumentResources {
		document_id: DocumentId,
	},
	LoadFont {
		font: Font,
	},
	NewDocumentWithName {
		name: String,
	},
	NextDocument,
	OpenDocument,
	OpenDocumentFile {
		document_name: String,
		document_serialized_content: String,
	},
	ToggleResetNodesToDefinitionsOnOpen,
	OpenDocumentFileWithId {
		document_id: DocumentId,
		document_name: String,
		document_is_auto_saved: bool,
		document_is_saved: bool,
		document_serialized_content: String,
		to_front: bool,
	},
	PasteIntoFolder {
		clipboard: Clipboard,
		parent: LayerNodeIdentifier,
		insert_index: usize,
	},
	PasteSerializedData {
		data: String,
	},
	CenterPastedLayers {
		layers: Vec<LayerNodeIdentifier>,
	},
	PasteImage {
		name: Option<String>,
		image: Image<Color>,
		mouse: Option<(f64, f64)>,
		parent_and_insert_index: Option<(LayerNodeIdentifier, usize)>,
	},
	PasteSvg {
		name: Option<String>,
		svg: String,
		mouse: Option<(f64, f64)>,
		parent_and_insert_index: Option<(LayerNodeIdentifier, usize)>,
	},
	PrevDocument,
	SetActivePanel {
		panel: PanelType,
	},
	SetDevicePixelRatio {
		ratio: f64,
	},
	SelectDocument {
		document_id: DocumentId,
	},
	ToggleRulers,
	UpdateDocumentWidgets,
	UpdateOpenDocumentsList,
}
