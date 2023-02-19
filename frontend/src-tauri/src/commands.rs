use crate::helpers::translate_key;
use editor::messages::layout::utility_types::misc::LayoutTarget;
use graphite_editor::application::Editor;
use graphite_editor::messages::frontend::utility_types::FrontendImageData;

use document_legacy::LayerId;
use graph_craft::document::NodeId;
use graphene_core::raster::color::Color;
use graphite_editor as editor;
use graphite_editor::application::generate_uuid;
use graphite_editor::consts::{FILE_SAVE_SUFFIX, GRAPHITE_DOCUMENT_VERSION};
use graphite_editor::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
use graphite_editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use graphite_editor::messages::portfolio::utility_types::{ImaginateServerStatus, Platform};
use graphite_editor::messages::prelude::*;

fn dispatch(message: impl Into<Message>) -> Vec<FrontendMessage> {
	let mut guard = crate::EDITOR.lock().unwrap();
	let editor = (*guard).as_mut().unwrap();
	let responses = editor.handle_message(message);
	responses
}

#[tauri::command]
pub fn init_after_frontend_ready(platform: String) -> Vec<FrontendMessage> {
	let platform = match platform.as_str() {
		"Windows" => Platform::Windows,
		"Mac" => Platform::Mac,
		"Linux" => Platform::Linux,
		_ => Platform::Unknown,
	};

	let mut responses = dispatch(GlobalsMessage::SetPlatform { platform });
	responses.extend(dispatch(Message::Init));
	responses
}

/// Displays a dialog with an error message
#[tauri::command]
pub fn error_dialog(title: String, description: String) -> Vec<FrontendMessage> {
	let message = DialogMessage::DisplayDialogError { title, description };
	dispatch(message)
}

/// Answer whether or not the editor has crashed
#[tauri::command]
pub fn has_crashed() -> bool {
	false
}

/// Answer whether or not the editor is in development mode
#[tauri::command]
pub fn in_development_mode() -> bool {
	cfg!(debug_assertions)
}

/// Get the constant `FILE_SAVE_SUFFIX`
#[tauri::command]
pub fn file_save_suffix() -> String {
	FILE_SAVE_SUFFIX.into()
}

/// Get the constant `GRAPHITE_DOCUMENT_VERSION`
#[tauri::command]
pub fn graphite_document_version() -> String {
	GRAPHITE_DOCUMENT_VERSION.to_string()
}

/// Update layout of a given UI
#[tauri::command]
#[specta::specta]
pub fn update_layout(layout_target: LayoutTarget, widget_id: u64, value: String) -> Result<Vec<FrontendMessage>, String> {
	log::debug!("{}", value);
	let value = serde_json::from_str(&value).unwrap_or(serde_json::Value::Null);
	let message = LayoutMessage::UpdateLayout { layout_target, widget_id, value };
	Ok(dispatch(message))
}

#[tauri::command]
pub fn load_preferences(preferences: String) -> Vec<FrontendMessage> {
	let message = PreferencesMessage::Load { preferences };

	dispatch(message)
}

#[tauri::command]
pub fn select_document(document_id: u64) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::SelectDocument { document_id };
	dispatch(message)
}

#[tauri::command]
pub fn new_document_dialog() -> Vec<FrontendMessage> {
	let message = DialogMessage::RequestNewDocumentDialog;
	dispatch(message)
}

#[tauri::command]
pub fn document_open() -> Vec<FrontendMessage> {
	let message = PortfolioMessage::OpenDocument;
	dispatch(message)
}

#[tauri::command]
pub fn open_document_file(document_name: String, document_serialized_content: String) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::OpenDocumentFile {
		document_name,
		document_serialized_content,
	};
	dispatch(message)
}

#[tauri::command]
pub fn open_auto_saved_document(document_id: u64, document_name: String, document_is_saved: bool, document_serialized_content: String) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::OpenDocumentFileWithId {
		document_id,
		document_name,
		document_is_auto_saved: true,
		document_is_saved,
		document_serialized_content,
	};
	dispatch(message)
}

#[tauri::command]
pub fn trigger_auto_save(document_id: u64) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::AutoSaveDocument { document_id };
	dispatch(message)
}

#[tauri::command]
pub fn close_document_with_confirmation(document_id: u64) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::CloseDocumentWithConfirmation { document_id };
	dispatch(message)
}

#[tauri::command]
pub fn request_about_graphite_dialog_with_localized_commit_date(localized_commit_date: String) -> Vec<FrontendMessage> {
	let message = DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate { localized_commit_date };
	dispatch(message)
}

/// Send new bounds when document panel viewports get resized or moved within the editor
/// [left, top, right, bottom]...
#[tauri::command]
pub fn bounds_of_viewports(bounds_of_viewports: Vec<f64>) -> Vec<FrontendMessage> {
	let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();

	let message = InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports: chunked };
	dispatch(message)
}

/// Mouse movement within the screenspace bounds of the viewport
#[tauri::command]
pub fn on_mouse_move(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Vec<FrontendMessage> {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };
	dispatch(message)
}

/// Mouse scrolling within the screenspace bounds of the viewport
#[tauri::command]
pub fn on_wheel_scroll(x: f64, y: f64, mouse_keys: u8, wheel_delta_x: i32, wheel_delta_y: i32, wheel_delta_z: i32, modifiers: u8) -> Vec<FrontendMessage> {
	let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
	editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys };
	dispatch(message)
}

/// A mouse button depressed within screenspace the bounds of the viewport
#[tauri::command]
pub fn on_mouse_down(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Vec<FrontendMessage> {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };
	dispatch(message)
}

/// A mouse button released
#[tauri::command]
pub fn on_mouse_up(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Vec<FrontendMessage> {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };
	dispatch(message)
}

/// Mouse double clicked
#[tauri::command]
pub fn on_double_click(x: f64, y: f64, mouse_keys: u8, modifiers: u8) -> Vec<FrontendMessage> {
	let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	let message = InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys };
	dispatch(message)
}

/// A keyboard button depressed within screenspace the bounds of the viewport
#[tauri::command]
pub fn on_key_down(name: String, modifiers: u8) -> Vec<FrontendMessage> {
	let key = translate_key(&name);
	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	trace!("Key down {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

	let message = InputPreprocessorMessage::KeyDown { key, modifier_keys };
	dispatch(message)
}

/// A keyboard button released
#[tauri::command]
pub fn on_key_up(name: String, modifiers: u8) -> Vec<FrontendMessage> {
	let key = translate_key(&name);
	let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

	trace!("Key up {:?}, name: {}, modifiers: {:?}", key, name, modifier_keys);

	let message = InputPreprocessorMessage::KeyUp { key, modifier_keys };
	dispatch(message)
}

/// A text box was committed
#[tauri::command]
pub fn on_change_text(new_text: String) -> Result<Vec<FrontendMessage>, String> {
	let message = TextToolMessage::TextChange { new_text };
	Ok(dispatch(message))
}

/// A font has been downloaded
#[tauri::command]
pub fn on_font_load(font_family: String, font_style: String, preview_url: String, data: Vec<u8>, is_default: bool) -> Result<Vec<FrontendMessage>, String> {
	let message = PortfolioMessage::FontLoaded {
		font_family,
		font_style,
		preview_url,
		data,
		is_default,
	};
	Ok(dispatch(message))
}

/// A text box was changed
#[tauri::command]
pub fn update_bounds(new_text: String) -> Result<Vec<FrontendMessage>, String> {
	let message = TextToolMessage::UpdateBounds { new_text };
	Ok(dispatch(message))
}

/// Begin sampling a pixel color from the document by entering eyedropper sampling mode
#[tauri::command]
pub fn eyedropper_sample_for_color_picker() -> Result<Vec<FrontendMessage>, String> {
	let message = DialogMessage::RequestComingSoonDialog { issue: Some(832) };
	Ok(dispatch(message))
}

/// Update primary color with values on a scale from 0 to 1.
#[tauri::command]
pub fn update_primary_color(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Vec<FrontendMessage>, String> {
	let primary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
		Some(color) => color,
		None => return Err("Invalid color".into()),
	};

	let message = ToolMessage::SelectPrimaryColor { color: primary_color };
	Ok(dispatch(message))
}

/// Update secondary color with values on a scale from 0 to 1.
#[tauri::command]
pub fn update_secondary_color(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Vec<FrontendMessage>, String> {
	let secondary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
		Some(color) => color,
		None => return Err("Invalid color".into()),
	};

	let message = ToolMessage::SelectSecondaryColor { color: secondary_color };
	Ok(dispatch(message))
}

/// Paste layers from a serialized json representation
#[tauri::command]
pub fn paste_serialized_data(data: String) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::PasteSerializedData { data };
	dispatch(message)
}

/// Modify the layer selection based on the layer which is clicked while holding down the <kbd>Ctrl</kbd> and/or <kbd>Shift</kbd> modifier keys used for range selection behavior
#[tauri::command]
pub fn select_layer(layer_path: Vec<LayerId>, ctrl: bool, shift: bool) -> Vec<FrontendMessage> {
	let message = DocumentMessage::SelectLayer { layer_path, ctrl, shift };
	dispatch(message)
}

/// Deselect all layers
#[tauri::command]
pub fn deselect_all_layers() -> Vec<FrontendMessage> {
	let message = DocumentMessage::DeselectAllLayers;
	dispatch(message)
}

/// Move a layer to be next to the specified neighbor
#[tauri::command]
pub fn move_layer_in_tree(folder_path: Vec<LayerId>, insert_index: isize) -> Vec<FrontendMessage> {
	let message = DocumentMessage::MoveSelectedLayersTo {
		folder_path,
		insert_index,
		reverse_index: true,
	};
	dispatch(message)
}

/// Set the name for the layer
#[tauri::command]
pub fn set_layer_name(layer_path: Vec<LayerId>, name: String) -> Vec<FrontendMessage> {
	let message = DocumentMessage::SetLayerName { layer_path, name };
	dispatch(message)
}

/// Translates document (in viewport coords)
#[tauri::command]
pub fn translate_canvas(delta_x: f64, delta_y: f64) -> Vec<FrontendMessage> {
	let message = NavigationMessage::TranslateCanvas { delta: (delta_x, delta_y).into() };
	dispatch(message)
}

/// Translates document (in viewport coords)
#[tauri::command]
pub fn translate_canvas_by_fraction(delta_x: f64, delta_y: f64) -> Vec<FrontendMessage> {
	let message = NavigationMessage::TranslateCanvasByViewportFraction { delta: (delta_x, delta_y).into() };
	dispatch(message)
}

/// Sends the blob URL generated by JS to the Image layer
#[tauri::command]
pub fn set_image_blob_url(document_id: u64, layer_path: Vec<LayerId>, blob_url: String, width: f64, height: f64) -> Vec<FrontendMessage> {
	let resolution = (width, height);
	let message = PortfolioMessage::SetImageBlobUrl {
		document_id,
		layer_path,
		blob_url,
		resolution,
	};
	dispatch(message)
}

/// Sends the blob URL generated by JS to the Imaginate layer in the respective document
#[tauri::command]
pub fn set_imaginate_image_data(document_id: u64, layer_path: Vec<LayerId>, node_path: Vec<NodeId>, image_data: Vec<u8>, width: u32, height: u32) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::ImaginateSetImageData {
		document_id,
		node_path,
		layer_path,
		image_data,
		width,
		height,
	};
	dispatch(message)
}

/// Notifies the Imaginate layer of a new percentage of completion and whether or not it's currently generating
#[tauri::command]
pub fn set_imaginate_generating_status(document_id: u64, layer_path: Vec<LayerId>, node_path: Vec<NodeId>, percent: Option<f64>, status: String) -> Vec<FrontendMessage> {
	use graph_craft::imaginate_input::ImaginateStatus;

	let status = match status.as_str() {
		"Idle" => ImaginateStatus::Idle,
		"Beginning" => ImaginateStatus::Beginning,
		"Uploading" => ImaginateStatus::Uploading(percent.expect("Percent needs to be supplied to set ImaginateStatus::Uploading")),
		"Generating" => ImaginateStatus::Generating,
		"Terminating" => ImaginateStatus::Terminating,
		"Terminated" => ImaginateStatus::Terminated,
		_ => panic!("Invalid string from JS for ImaginateStatus, received: {}", status),
	};

	let percent = if matches!(status, ImaginateStatus::Uploading(_)) { None } else { percent };

	let message = PortfolioMessage::ImaginateSetGeneratingStatus {
		document_id,
		layer_path,
		node_path,
		percent,
		status,
	};
	dispatch(message)
}

/// Notifies the editor that the Imaginate server is available or unavailable
#[tauri::command]
pub fn set_imaginate_server_status(available: bool) -> Vec<FrontendMessage> {
	let message: Message = match available {
		true => PortfolioMessage::ImaginateSetServerStatus {
			status: ImaginateServerStatus::Connected,
		}
		.into(),
		false => PortfolioMessage::ImaginateSetServerStatus {
			status: ImaginateServerStatus::Unavailable,
		}
		.into(),
	};
	dispatch(message)
}

/// Sends the blob URL generated by JS to the Imaginate layer in the respective document
#[tauri::command]
pub fn process_node_graph_frame(document_id: u64, layer_path: Vec<LayerId>, image_data: Vec<u8>, width: u32, height: u32, imaginate_node: Option<Vec<NodeId>>) -> Vec<FrontendMessage> {
	let message = PortfolioMessage::ProcessNodeGraphFrame {
		document_id,
		layer_path,
		image_data,
		size: (width, height),
		imaginate_node,
	};
	dispatch(message)
}

/// Notifies the backend that the user connected a node's primary output to one of another node's inputs
#[tauri::command]
pub fn connect_nodes_by_link(output_node: u64, output_node_connector_index: usize, input_node: u64, input_node_connector_index: usize) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::ConnectNodesByLink {
		output_node,
		output_node_connector_index,
		input_node,
		input_node_connector_index,
	};
	dispatch(message)
}

/// Shifts the node and its children to stop nodes going ontop of each other
#[tauri::command]
pub fn shift_node(node_id: u64) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::ShiftNode { node_id };
	dispatch(message)
}

/// Notifies the backend that the user disconnected a node
#[tauri::command]
pub fn disconnect_nodes(node_id: u64, input_index: usize) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::DisconnectNodes { node_id, input_index };
	dispatch(message)
}

/// Check for intersections between the curve and a rectangle defined by opposite corners
#[tauri::command]
pub fn rectangle_intersects(bezier_x: Vec<f64>, bezier_y: Vec<f64>, top: f64, left: f64, bottom: f64, right: f64) -> bool {
	let bezier = bezier_rs::Bezier::from_cubic_dvec2(
		(bezier_x[0], bezier_y[0]).into(),
		(bezier_x[1], bezier_y[1]).into(),
		(bezier_x[2], bezier_y[2]).into(),
		(bezier_x[3], bezier_y[3]).into(),
	);
	!bezier.rectangle_intersections((left, top).into(), (right, bottom).into()).is_empty() || bezier.is_contained_within((left, top).into(), (right, bottom).into())
}

/// Creates a new document node in the node graph
#[tauri::command]
pub fn create_node(node_type: String, x: i32, y: i32) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::CreateNode { node_id: None, node_type, x, y };
	dispatch(message)
}

/// Notifies the backend that the user selected a node in the node graph
#[tauri::command]
pub fn select_nodes(nodes: Vec<u64>) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::SelectNodes { nodes };
	dispatch(message)
}

/// Pastes the nodes based on serialized data
#[tauri::command]
pub fn paste_serialized_nodes(serialized_nodes: String) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::PasteNodes { serialized_nodes };
	dispatch(message)
}

/// Notifies the backend that the user double clicked a node
#[tauri::command]
pub fn double_click_node(node: u64) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::DoubleClickNode { node };
	dispatch(message)
}

/// Notifies the backend that the selected nodes have been moved
#[tauri::command]
pub fn move_selected_nodes(displacement_x: i32, displacement_y: i32) -> Vec<FrontendMessage> {
	let message = DocumentMessage::StartTransaction;
	let mut responses = dispatch(message);

	let message = NodeGraphMessage::MoveSelectedNodes { displacement_x, displacement_y };
	responses.extend(dispatch(message));
	responses
}

/// Toggle preview on node
#[tauri::command]
pub fn toggle_preview(node_id: NodeId) -> Vec<FrontendMessage> {
	let message = NodeGraphMessage::TogglePreview { node_id };
	dispatch(message)
}

/// Pastes an image
#[tauri::command]
pub fn paste_image(image_data: Vec<u8>, width: u32, height: u32, mouse_x: Option<f64>, mouse_y: Option<f64>) -> Vec<FrontendMessage> {
	let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));
	let image = graphene_core::raster::Image::from_image_data(&image_data, width, height);
	let message = DocumentMessage::PasteImage { image, mouse };
	dispatch(message)
}

/// Toggle visibility of a layer from the layer list
#[tauri::command]
pub fn toggle_layer_visibility(layer_path: Vec<LayerId>) -> Vec<FrontendMessage> {
	let message = DocumentMessage::ToggleLayerVisibility { layer_path };
	dispatch(message)
}

/// Toggle expansions state of a layer from the layer list
#[tauri::command]
pub fn toggle_layer_expansion(layer_path: Vec<LayerId>) -> Vec<FrontendMessage> {
	let message = DocumentMessage::ToggleLayerExpansion { layer_path };
	dispatch(message)
}
