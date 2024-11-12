#![allow(clippy::too_many_arguments)]
//
// This file is where functions are defined to be called directly from JS.
// It serves as a thin wrapper over the editor backend API that relies
// on the dispatcher messaging system and more complex Rust data types.
//
use crate::helpers::translate_key;
use crate::{Error, EDITOR, EDITOR_HANDLE, EDITOR_HAS_CRASHED};

use editor::application::Editor;
use editor::consts::FILE_SAVE_SUFFIX;
use editor::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
use editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use editor::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use editor::messages::portfolio::utility_types::Platform;
use editor::messages::prelude::*;
use editor::messages::tool::tool_messages::tool_prelude::WidgetId;
use graph_craft::document::NodeId;
use graphene_core::raster::color::Color;

use serde::Serialize;
use serde_wasm_bindgen::{self, from_value};
use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::time::Duration;
use wasm_bindgen::prelude::*;

/// We directly interface with the updateImage JS function for massively increased performance over serializing and deserializing.
/// This avoids creating a json with a list millions of numbers long.
// #[wasm_bindgen(module = "/../src/wasm-communication/editor.ts")]
// extern "C" {
// 	// fn dispatchTauri(message: String) -> String;
// 	fn dispatchTauri(message: String);
// }

/// Set the random seed used by the editor by calling this from JS upon initialization.
/// This is necessary because WASM doesn't have a random number generator.
#[wasm_bindgen(js_name = setRandomSeed)]
pub fn set_random_seed(seed: u64) {
	editor::application::set_uuid_seed(seed);
}

/// Provides a handle to access the raw WASM memory.
#[wasm_bindgen(js_name = wasmMemory)]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
}

// ============================================================================

/// This struct is, via wasm-bindgen, used by JS to interact with the editor backend. It does this by calling functions, which are `impl`ed
#[wasm_bindgen]
#[derive(Clone)]
pub struct EditorHandle {
	/// This callback is called by the editor's dispatcher when directing FrontendMessages from Rust to JS
	frontend_message_handler_callback: js_sys::Function,
}

// Defined separately from the `impl` block below since this `impl` block lacks the `#[wasm_bindgen]` attribute.
// Quirks in wasm-bindgen prevent functions in `#[wasm_bindgen]` `impl` blocks from being made publicly accessible from Rust.
impl EditorHandle {
	pub fn send_frontend_message_to_js_rust_proxy(&self, message: FrontendMessage) {
		self.send_frontend_message_to_js(message);
	}
}

#[wasm_bindgen]
impl EditorHandle {
	#[wasm_bindgen(constructor)]
	pub fn new(frontend_message_handler_callback: js_sys::Function) -> Self {
		let editor = Editor::new();
		let editor_handle = EditorHandle { frontend_message_handler_callback };
		if EDITOR.with(|handle| handle.lock().ok().map(|mut guard| *guard = Some(editor))).is_none() {
			log::error!("Attempted to initialize the editor more than once");
		}
		if EDITOR_HANDLE.with(|handle| handle.lock().ok().map(|mut guard| *guard = Some(editor_handle.clone()))).is_none() {
			log::error!("Attempted to initialize the editor handle more than once");
		}
		editor_handle
	}

	// Sends a message to the dispatcher in the Editor Backend
	fn dispatch<T: Into<Message>>(&self, message: T) {
		// Process no further messages after a crash to avoid spamming the console
		if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
			return;
		}

		// Get the editor, dispatch the message, and store the `FrontendMessage` queue response
		let frontend_messages = editor(|editor| editor.handle_message(message.into()));

		// Send each `FrontendMessage` to the JavaScript frontend
		for message in frontend_messages.into_iter() {
			self.send_frontend_message_to_js(message);
		}
	}

	// Sends a FrontendMessage to JavaScript
	fn send_frontend_message_to_js(&self, mut message: FrontendMessage) {
		if let FrontendMessage::UpdateDocumentLayerStructure { data_buffer } = message {
			message = FrontendMessage::UpdateDocumentLayerStructureJs { data_buffer: data_buffer.into() };
		}

		let message_type = message.to_discriminant().local_name();

		let serializer = serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
		let message_data = message.serialize(&serializer).expect("Failed to serialize FrontendMessage");

		let js_return_value = self.frontend_message_handler_callback.call2(&JsValue::null(), &JsValue::from(message_type), &message_data);

		if let Err(error) = js_return_value {
			error!(
				"While handling FrontendMessage \"{:?}\", JavaScript threw an error: {:?}",
				message.to_discriminant().local_name(),
				error,
			)
		}
	}

	// ========================================================================
	// Add additional JS -> Rust wrapper functions below as needed for calling
	// the backend from the web frontend.
	// ========================================================================

	#[wasm_bindgen(js_name = initAfterFrontendReady)]
	pub fn init_after_frontend_ready(&self, platform: String) {
		// Send initialization messages
		let platform = match platform.as_str() {
			"Windows" => Platform::Windows,
			"Mac" => Platform::Mac,
			"Linux" => Platform::Linux,
			_ => Platform::Unknown,
		};
		self.dispatch(GlobalsMessage::SetPlatform { platform });
		self.dispatch(Message::Init);

		// Poll node graph evaluation on `requestAnimationFrame`
		{
			let f = std::rc::Rc::new(RefCell::new(None));
			let g = f.clone();

			*g.borrow_mut() = Some(Closure::new(move |timestamp| {
				wasm_bindgen_futures::spawn_local(poll_node_graph_evaluation());

				if !EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
					editor_and_handle(|editor, handle| {
						let micros: f64 = timestamp * 1000.;
						let timestamp = Duration::from_micros(micros.round() as u64);

						for message in editor.handle_message(InputPreprocessorMessage::FrameTimeAdvance { timestamp }) {
							handle.send_frontend_message_to_js(message);
						}

						for message in editor.handle_message(BroadcastMessage::TriggerEvent(BroadcastEvent::AnimationFrame)) {
							handle.send_frontend_message_to_js(message);
						}
					});
				}

				// Schedule ourself for another requestAnimationFrame callback
				request_animation_frame(f.borrow().as_ref().unwrap());
			}));

			request_animation_frame(g.borrow().as_ref().unwrap());
		}

		// Auto save all documents on `setTimeout`
		{
			let f = std::rc::Rc::new(RefCell::new(None));
			let g = f.clone();

			*g.borrow_mut() = Some(Closure::new(move || {
				auto_save_all_documents();

				// Schedule ourself for another setTimeout callback
				set_timeout(f.borrow().as_ref().unwrap(), Duration::from_secs(editor::consts::AUTO_SAVE_TIMEOUT_SECONDS));
			}));

			set_timeout(g.borrow().as_ref().unwrap(), Duration::from_secs(editor::consts::AUTO_SAVE_TIMEOUT_SECONDS));
		}
	}

	// #[wasm_bindgen(js_name = tauriResponse)]
	// pub fn tauri_response(&self, _message: JsValue) {
	// 	#[cfg(feature = "tauri")]
	// 	match ron::from_str::<Vec<FrontendMessage>>(&_message.as_string().unwrap()) {
	// 		Ok(response) => {
	// 			for message in response {
	// 				self.send_frontend_message_to_js(message);
	// 			}
	// 		}
	// 		Err(error) => {
	// 			log::error!("tauri response: {error:?}\n{_message:?}");
	// 		}
	// 	}
	// }

	/// Displays a dialog with an error message
	#[wasm_bindgen(js_name = errorDialog)]
	pub fn error_dialog(&self, title: String, description: String) {
		let message = DialogMessage::DisplayDialogError { title, description };
		self.dispatch(message);
	}

	/// Answer whether or not the editor has crashed
	#[wasm_bindgen(js_name = hasCrashed)]
	pub fn has_crashed(&self) -> bool {
		EDITOR_HAS_CRASHED.load(Ordering::SeqCst)
	}

	/// Answer whether or not the editor is in development mode
	#[wasm_bindgen(js_name = inDevelopmentMode)]
	pub fn in_development_mode(&self) -> bool {
		cfg!(debug_assertions)
	}

	/// Get the constant `FILE_SAVE_SUFFIX`
	#[wasm_bindgen(js_name = fileSaveSuffix)]
	pub fn file_save_suffix(&self) -> String {
		FILE_SAVE_SUFFIX.into()
	}

	/// Update the value of a given UI widget, but don't commit it to the history (unless `commit_layout()` is called, which handles that)
	#[wasm_bindgen(js_name = widgetValueUpdate)]
	pub fn widget_value_update(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		let widget_id = WidgetId(widget_id);
		match (from_value(layout_target), from_value(value)) {
			(Ok(layout_target), Ok(value)) => {
				let message = LayoutMessage::WidgetValueUpdate { layout_target, widget_id, value };
				self.dispatch(message);
				Ok(())
			}
			(target, val) => Err(Error::new(&format!("Could not update UI\nDetails:\nTarget: {target:?}\nValue: {val:?}")).into()),
		}
	}

	/// Commit the value of a given UI widget to the history
	#[wasm_bindgen(js_name = widgetValueCommit)]
	pub fn widget_value_commit(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		let widget_id = WidgetId(widget_id);
		match (from_value(layout_target), from_value(value)) {
			(Ok(layout_target), Ok(value)) => {
				let message = LayoutMessage::WidgetValueCommit { layout_target, widget_id, value };
				self.dispatch(message);
				Ok(())
			}
			(target, val) => Err(Error::new(&format!("Could not commit UI\nDetails:\nTarget: {target:?}\nValue: {val:?}")).into()),
		}
	}

	/// Update the value of a given UI widget, and commit it to the history
	#[wasm_bindgen(js_name = widgetValueCommitAndUpdate)]
	pub fn widget_value_commit_and_update(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		self.widget_value_commit(layout_target.clone(), widget_id, value.clone())?;
		self.widget_value_update(layout_target, widget_id, value)?;
		Ok(())
	}

	#[wasm_bindgen(js_name = loadPreferences)]
	pub fn load_preferences(&self, preferences: String) {
		let message = PreferencesMessage::Load { preferences };

		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = selectDocument)]
	pub fn select_document(&self, document_id: u64) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::SelectDocument { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = newDocumentDialog)]
	pub fn new_document_dialog(&self) {
		let message = DialogMessage::RequestNewDocumentDialog;
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = openDocument)]
	pub fn open_document(&self) {
		let message = PortfolioMessage::OpenDocument;
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = demoArtworkDialog)]
	pub fn demo_artwork_dialog(&self) {
		let message = DialogMessage::RequestDemoArtworkDialog;
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = openDocumentFile)]
	pub fn open_document_file(&self, document_name: String, document_serialized_content: String) {
		let message = PortfolioMessage::OpenDocumentFile {
			document_name,
			document_serialized_content,
		};
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = openAutoSavedDocument)]
	pub fn open_auto_saved_document(&self, document_id: u64, document_name: String, document_is_saved: bool, document_serialized_content: String) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::OpenDocumentFileWithId {
			document_id,
			document_name,
			document_is_auto_saved: true,
			document_is_saved,
			document_serialized_content,
		};
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = triggerAutoSave)]
	pub fn trigger_auto_save(&self, document_id: u64) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::AutoSaveDocument { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = closeDocumentWithConfirmation)]
	pub fn close_document_with_confirmation(&self, document_id: u64) {
		let document_id = DocumentId(document_id);
		let message = PortfolioMessage::CloseDocumentWithConfirmation { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = requestAboutGraphiteDialogWithLocalizedCommitDate)]
	pub fn request_about_graphite_dialog_with_localized_commit_date(&self, localized_commit_date: String, localized_commit_year: String) {
		let message = DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate {
			localized_commit_date,
			localized_commit_year,
		};
		self.dispatch(message);
	}

	/// Send new bounds when document panel viewports get resized or moved within the editor
	/// [left, top, right, bottom]...
	#[wasm_bindgen(js_name = boundsOfViewports)]
	pub fn bounds_of_viewports(&self, bounds_of_viewports: &[f64]) {
		let chunked: Vec<_> = bounds_of_viewports.chunks(4).map(ViewportBounds::from_slice).collect();

		let message = InputPreprocessorMessage::BoundsOfViewports { bounds_of_viewports: chunked };
		self.dispatch(message);
	}

	/// Zoom the canvas to fit all content
	#[wasm_bindgen(js_name = zoomCanvasToFitAll)]
	pub fn zoom_canvas_to_fit_all(&self) {
		let message = DocumentMessage::ZoomCanvasToFitAll;
		self.dispatch(message);
	}

	/// Mouse movement within the screenspace bounds of the viewport
	#[wasm_bindgen(js_name = onMouseMove)]
	pub fn on_mouse_move(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// Mouse scrolling within the screenspace bounds of the viewport
	#[wasm_bindgen(js_name = onWheelScroll)]
	pub fn on_wheel_scroll(&self, x: f64, y: f64, mouse_keys: u8, wheel_delta_x: f64, wheel_delta_y: f64, wheel_delta_z: f64, modifiers: u8) {
		let mut editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
		editor_mouse_state.scroll_delta = ScrollDelta::new(wheel_delta_x, wheel_delta_y, wheel_delta_z);

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::WheelScroll { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A mouse button depressed within screenspace the bounds of the viewport
	#[wasm_bindgen(js_name = onMouseDown)]
	pub fn on_mouse_down(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A mouse button released
	#[wasm_bindgen(js_name = onMouseUp)]
	pub fn on_mouse_up(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// Mouse double clicked
	#[wasm_bindgen(js_name = onDoubleClick)]
	pub fn on_double_click(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
		let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());

		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		let message = InputPreprocessorMessage::DoubleClick { editor_mouse_state, modifier_keys };
		self.dispatch(message);
	}

	/// A keyboard button depressed within screenspace the bounds of the viewport
	#[wasm_bindgen(js_name = onKeyDown)]
	pub fn on_key_down(&self, name: String, modifiers: u8, key_repeat: bool) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		trace!("Key down {key:?}, name: {name}, modifiers: {modifiers:?}, key repeat: {key_repeat}");

		let message = InputPreprocessorMessage::KeyDown { key, key_repeat, modifier_keys };
		self.dispatch(message);
	}

	/// A keyboard button released
	#[wasm_bindgen(js_name = onKeyUp)]
	pub fn on_key_up(&self, name: String, modifiers: u8, key_repeat: bool) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		trace!("Key up {key:?}, name: {name}, modifiers: {modifier_keys:?}, key repeat: {key_repeat}");

		let message = InputPreprocessorMessage::KeyUp { key, key_repeat, modifier_keys };
		self.dispatch(message);
	}

	/// A text box was committed
	#[wasm_bindgen(js_name = onChangeText)]
	pub fn on_change_text(&self, new_text: String) -> Result<(), JsValue> {
		let message = TextToolMessage::TextChange { new_text };
		self.dispatch(message);

		Ok(())
	}

	/// A font has been downloaded
	#[wasm_bindgen(js_name = onFontLoad)]
	pub fn on_font_load(&self, font_family: String, font_style: String, preview_url: String, data: Vec<u8>) -> Result<(), JsValue> {
		let message = PortfolioMessage::FontLoaded {
			font_family,
			font_style,
			preview_url,
			data,
		};
		self.dispatch(message);

		Ok(())
	}

	/// A text box was changed
	#[wasm_bindgen(js_name = updateBounds)]
	pub fn update_bounds(&self, new_text: String) -> Result<(), JsValue> {
		let message = TextToolMessage::UpdateBounds { new_text };
		self.dispatch(message);

		Ok(())
	}

	/// Begin sampling a pixel color from the document by entering eyedropper sampling mode
	#[wasm_bindgen(js_name = eyedropperSampleForColorPicker)]
	pub fn eyedropper_sample_for_color_picker(&self) -> Result<(), JsValue> {
		let message = DialogMessage::RequestComingSoonDialog { issue: Some(832) };
		self.dispatch(message);

		Ok(())
	}

	/// Update primary color with values on a scale from 0 to 1.
	#[wasm_bindgen(js_name = updatePrimaryColor)]
	pub fn update_primary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let primary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectPrimaryColor { color: primary_color };
		self.dispatch(message);

		Ok(())
	}

	/// Update secondary color with values on a scale from 0 to 1.
	#[wasm_bindgen(js_name = updateSecondaryColor)]
	pub fn update_secondary_color(&self, red: f32, green: f32, blue: f32, alpha: f32) -> Result<(), JsValue> {
		let secondary_color = match Color::from_rgbaf32(red, green, blue, alpha) {
			Some(color) => color,
			None => return Err(Error::new("Invalid color").into()),
		};

		let message = ToolMessage::SelectSecondaryColor { color: secondary_color };
		self.dispatch(message);

		Ok(())
	}

	/// Paste layers from a serialized json representation
	#[wasm_bindgen(js_name = pasteSerializedData)]
	pub fn paste_serialized_data(&self, data: String) {
		let message = PortfolioMessage::PasteSerializedData { data };
		self.dispatch(message);
	}

	/// Modify the layer selection based on the layer which is clicked while holding down the <kbd>Ctrl</kbd> and/or <kbd>Shift</kbd> modifier keys used for range selection behavior
	#[wasm_bindgen(js_name = selectLayer)]
	pub fn select_layer(&self, id: u64, ctrl: bool, shift: bool) {
		let id = NodeId(id);
		let message = DocumentMessage::SelectLayer { id, ctrl, shift };
		self.dispatch(message);
	}

	/// Deselect all layers
	#[wasm_bindgen(js_name = deselectAllLayers)]
	pub fn deselect_all_layers(&self) {
		let message = DocumentMessage::DeselectAllLayers;
		self.dispatch(message);
	}

	/// Move a layer to within a folder and placed down at the given index.
	/// If the folder is `None`, it is inserted into the document root.
	/// If the insert index is `None`, it is inserted at the start of the folder.
	#[wasm_bindgen(js_name = moveLayerInTree)]
	pub fn move_layer_in_tree(&self, insert_parent_id: Option<u64>, insert_index: Option<usize>) {
		let insert_parent_id = insert_parent_id.map(NodeId);
		let parent = insert_parent_id.map(LayerNodeIdentifier::new_unchecked).unwrap_or_default();

		let message = DocumentMessage::MoveSelectedLayersTo {
			parent,
			insert_index: insert_index.unwrap_or_default(),
		};
		self.dispatch(message);
	}

	/// Set the name for the layer
	#[wasm_bindgen(js_name = setLayerName)]
	pub fn set_layer_name(&self, id: u64, name: String) {
		let layer = LayerNodeIdentifier::new_unchecked(NodeId(id));
		let message = NodeGraphMessage::SetDisplayName {
			node_id: layer.to_node(),
			alias: name,
		};
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	#[wasm_bindgen(js_name = panCanvas)]
	pub fn translate_canvas(&self, delta_x: f64, delta_y: f64) {
		let message = NavigationMessage::CanvasPan { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	#[wasm_bindgen(js_name = panCanvasByFraction)]
	pub fn translate_canvas_by_fraction(&self, delta_x: f64, delta_y: f64) {
		let message = NavigationMessage::CanvasPanByViewportFraction { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Snaps the import/export edges to a grid space when the scroll bar is released
	#[wasm_bindgen(js_name = setGridAlignedEdges)]
	pub fn set_grid_aligned_edges(&self) {
		let message = NodeGraphMessage::SetGridAlignedEdges;
		self.dispatch(message);
	}

	/// Merge a group of nodes into a subnetwork
	#[wasm_bindgen(js_name = mergeSelectedNodes)]
	pub fn merge_nodes(&self) {
		let message = NodeGraphMessage::MergeSelectedNodes;
		self.dispatch(message);
	}

	/// Creates a new document node in the node graph
	#[wasm_bindgen(js_name = createNode)]
	pub fn create_node(&self, node_type: String, x: i32, y: i32) {
		let id = NodeId::new();
		let message = NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(id),
			node_type,
			xy: Some((x / 24, y / 24)),
		};
		self.dispatch(message);
	}

	/// Pastes the nodes based on serialized data
	#[wasm_bindgen(js_name = pasteSerializedNodes)]
	pub fn paste_serialized_nodes(&self, serialized_nodes: String) {
		let message = NodeGraphMessage::PasteNodes { serialized_nodes };
		self.dispatch(message);
	}

	/// Pastes an image
	#[wasm_bindgen(js_name = pasteImage)]
	pub fn paste_image(
		&self,
		name: Option<String>,
		image_data: Vec<u8>,
		width: u32,
		height: u32,
		mouse_x: Option<f64>,
		mouse_y: Option<f64>,
		insert_parent_id: Option<u64>,
		insert_index: Option<usize>,
	) {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));
		let image = graphene_core::raster::Image::from_image_data(&image_data, width, height);

		let parent_and_insert_index = if let (Some(insert_parent_id), Some(insert_index)) = (insert_parent_id, insert_index) {
			let insert_parent_id = NodeId(insert_parent_id);
			let parent = LayerNodeIdentifier::new_unchecked(insert_parent_id);
			Some((parent, insert_index))
		} else {
			None
		};

		let message = PortfolioMessage::PasteImage {
			name,
			image,
			mouse,
			parent_and_insert_index,
		};
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = pasteSvg)]
	pub fn paste_svg(&self, name: Option<String>, svg: String, mouse_x: Option<f64>, mouse_y: Option<f64>, insert_parent_id: Option<u64>, insert_index: Option<usize>) {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));

		let parent_and_insert_index = if let (Some(insert_parent_id), Some(insert_index)) = (insert_parent_id, insert_index) {
			let insert_parent_id = NodeId(insert_parent_id);
			let parent = LayerNodeIdentifier::new_unchecked(insert_parent_id);
			Some((parent, insert_index))
		} else {
			None
		};

		let message = PortfolioMessage::PasteSvg {
			name,
			svg,
			mouse,
			parent_and_insert_index,
		};
		self.dispatch(message);
	}

	/// Toggle visibility of a layer or node given its node ID
	#[wasm_bindgen(js_name = toggleNodeVisibilityLayerPanel)]
	pub fn toggle_node_visibility_layer(&self, id: u64) {
		let node_id = NodeId(id);
		let message = NodeGraphMessage::ToggleVisibility { node_id };
		self.dispatch(message);
	}

	/// Pin or unpin a node given its node ID
	#[wasm_bindgen(js_name = setNodePinned)]
	pub fn set_node_pinned(&self, id: u64, pinned: bool) {
		self.dispatch(DocumentMessage::SetNodePinned { node_id: NodeId(id), pinned });
	}

	/// Delete a layer or node given its node ID
	#[wasm_bindgen(js_name = deleteNode)]
	pub fn delete_node(&self, id: u64) {
		self.dispatch(DocumentMessage::DeleteNode { node_id: NodeId(id) });
	}

	/// Toggle lock state of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerLock)]
	pub fn toggle_layer_lock(&self, node_id: u64) {
		let message = NodeGraphMessage::ToggleLocked { node_id: NodeId(node_id) };
		self.dispatch(message);
	}

	/// Toggle expansions state of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerExpansion)]
	pub fn toggle_layer_expansion(&self, id: u64) {
		let id = NodeId(id);
		let message = DocumentMessage::ToggleLayerExpansion { id };
		self.dispatch(message);
	}

	/// Set the active panel to the most recently clicked panel
	#[wasm_bindgen(js_name = setActivePanel)]
	pub fn set_active_panel(&self, panel: String) {
		let message = PortfolioMessage::SetActivePanel { panel: panel.into() };
		self.dispatch(message);
	}

	/// Toggle display type for a layer
	#[wasm_bindgen(js_name = setToNodeOrLayer)]
	pub fn set_to_node_or_layer(&self, id: u64, is_layer: bool) {
		self.dispatch(DocumentMessage::SetToNodeOrLayer { node_id: NodeId(id), is_layer });
	}

	#[wasm_bindgen(js_name = injectImaginatePollServerStatus)]
	pub fn inject_imaginate_poll_server_status(&self) {
		self.dispatch(PortfolioMessage::ImaginatePollServerStatus);
	}

	// TODO: Eventually remove this (probably starting late 2024)
	#[wasm_bindgen(js_name = triggerUpgradeDocumentToVectorManipulationFormat)]
	pub async fn upgrade_document_to_vector_manipulation_format(
		&self,
		document_id: u64,
		document_name: String,
		document_is_auto_saved: bool,
		document_is_saved: bool,
		document_serialized_content: String,
	) {
		use editor::messages::portfolio::document::graph_operation::transform_utils::*;
		use editor::messages::portfolio::document::graph_operation::utility_types::*;
		use editor::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
		use editor::node_graph_executor::replace_node_runtime;
		use editor::node_graph_executor::NodeRuntime;
		use graph_craft::document::NodeInput;
		use graph_craft::document::{value::TaggedValue, DocumentNodeImplementation};
		use graphene_core::vector::*;

		let (_, request_receiver) = std::sync::mpsc::channel();
		let (response_sender, _) = std::sync::mpsc::channel();
		let old_runtime = replace_node_runtime(NodeRuntime::new(request_receiver, response_sender)).await;

		let mut editor = Editor::new();
		let document_id = DocumentId(document_id);
		editor.handle_message(PortfolioMessage::OpenDocumentFileWithId {
			document_id,
			document_name: document_name.clone(),
			document_is_auto_saved,
			document_is_saved,
			document_serialized_content: document_serialized_content.clone(),
		});

		let document = editor.dispatcher.message_handlers.portfolio_message_handler.active_document_mut().unwrap();
		for node in document
			.network_interface
			.network_metadata(&[])
			.unwrap()
			.persistent_metadata
			.node_metadata
			.iter()
			.filter(|(_, d)| d.persistent_metadata.reference.as_ref().is_some_and(|reference| reference == "Artboard"))
			.map(|(id, _)| *id)
			.collect::<Vec<_>>()
		{
			let Some(document_node) = document.network_interface.network(&[]).unwrap().nodes.get(&node) else {
				log::error!("Could not get document node in document network");
				return;
			};
			if let Some(network) = document_node.implementation.get_network() {
				let mut nodes_to_upgrade = Vec::new();
				for (node_id, _) in network.nodes.iter().collect::<Vec<_>>() {
					if document.network_interface.reference(node_id, &[]).is_some_and(|reference| reference == "To Artboard")
						&& document
							.network_interface
							.network(&[])
							.unwrap()
							.nodes
							.get(node_id)
							.is_some_and(|document_node| document_node.inputs.len() != 6)
					{
						nodes_to_upgrade.push(*node_id);
					}
				}
				for node_id in nodes_to_upgrade {
					document
						.network_interface
						.replace_implementation(&node_id, &[], DocumentNodeImplementation::proto("graphene_core::ToArtboardNode"));
					document.network_interface.add_import(TaggedValue::IVec2(glam::IVec2::default()), false, 2, "".to_string(), &[node_id]);
				}
			}
		}

		let portfolio = &mut editor.dispatcher.message_handlers.portfolio_message_handler;
		portfolio
			.executor
			.submit_node_graph_evaluation(portfolio.documents.get_mut(&portfolio.active_document_id().unwrap()).unwrap(), glam::UVec2::ONE, true)
			.unwrap();
		editor::node_graph_executor::run_node_graph().await;

		let mut messages = VecDeque::new();
		if let Err(err) = editor.poll_node_graph_evaluation(&mut messages) {
			log::warn!(
				"While attempting to upgrade the old document format, the graph evaluation failed which is necessary for the upgrade process:\n{:#?}",
				err
			);

			replace_node_runtime(old_runtime.unwrap()).await;

			let document_name = document_name.clone() + "__DO_NOT_UPGRADE__";
			self.dispatch(PortfolioMessage::OpenDocumentFileWithId {
				document_id,
				document_name,
				document_is_auto_saved,
				document_is_saved,
				document_serialized_content,
			});
			return;
		}

		let mut updated_nodes = HashSet::new();
		let document = editor.dispatcher.message_handlers.portfolio_message_handler.active_document_mut().unwrap();
		document.network_interface.load_structure();
		for node in document
			.network_interface
			.network_metadata(&[])
			.unwrap()
			.persistent_metadata
			.node_metadata
			.iter()
			.filter(|(_, d)| d.persistent_metadata.reference.as_ref().is_some_and(|reference| reference == "Merge"))
			.map(|(id, _)| *id)
			.collect::<Vec<_>>()
		{
			let layer = LayerNodeIdentifier::new(node, &document.network_interface, &[]);
			if layer.has_children(document.metadata()) {
				continue;
			}

			let bounds = LayerBounds::new(document.metadata(), layer);

			let mut responses = VecDeque::new();
			let mut shape = None;

			if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, &mut document.network_interface, &mut responses) {
				let Some(transform_node_id) = modify_inputs.existing_node_id("Transform") else {
					return;
				};
				if !updated_nodes.insert(transform_node_id) {
					return;
				}
				let Some(inputs) = modify_inputs.network_interface.network(&[]).unwrap().nodes.get(&transform_node_id).map(|node| &node.inputs) else {
					log::error!("Could not get transform node in document network");
					return;
				};
				let transform = get_current_transform(inputs);
				let upstream_transform = modify_inputs.network_interface.document_metadata().upstream_transform(transform_node_id);
				let pivot_transform = glam::DAffine2::from_translation(upstream_transform.transform_point2(bounds.local_pivot(get_current_normalized_pivot(inputs))));

				update_transform(&mut document.network_interface, &transform_node_id, pivot_transform * transform * pivot_transform.inverse());
			}
			if let Some(mut modify_inputs) = ModifyInputsContext::new_with_layer(layer, &mut document.network_interface, &mut responses) {
				let Some(shape_node_id) = modify_inputs.existing_node_id("Shape") else {
					return;
				};
				if !updated_nodes.insert(shape_node_id) {
					return;
				}
				let Some(shape_node) = modify_inputs.network_interface.network(&[]).unwrap().nodes.get(&shape_node_id) else {
					log::error!("Could not get shape node in document network");
					return;
				};
				let path_data = match &shape_node.inputs[0].as_value() {
					Some(TaggedValue::Subpaths(translation)) => translation,
					_ => &Vec::new(),
				};

				let colinear_manipulators = match &shape_node.inputs[1].as_value() {
					Some(TaggedValue::PointIds(translation)) => translation,
					_ => &Vec::new(),
				};

				let mut vector_data = VectorData::from_subpaths(path_data, false);
				vector_data.colinear_manipulators = colinear_manipulators
					.iter()
					.filter_map(|&point| ManipulatorPointId::Anchor(point).get_handle_pair(&vector_data))
					.collect();

				shape = Some((shape_node_id, VectorModification::create_from_vector(&vector_data)));
			}

			if let Some((node_id, modification)) = shape {
				let node_type = resolve_document_node_type("Path").unwrap();
				let document_node = node_type
					.node_template_input_override([None, Some(NodeInput::value(TaggedValue::VectorModification(Box::new(modification)), false))])
					.document_node;

				let node_metadata = document.network_interface.node_metadata(&node_id, &[]).cloned().unwrap_or_default();

				document.network_interface.insert_node(
					node_id,
					NodeTemplate {
						document_node,
						persistent_node_metadata: node_metadata.persistent_metadata,
					},
					&[],
				);
			}
		}

		let document_serialized_content = editor.dispatcher.message_handlers.portfolio_message_handler.active_document_mut().unwrap().serialize_document();

		replace_node_runtime(old_runtime.unwrap()).await;

		self.dispatch(PortfolioMessage::OpenDocumentFileWithId {
			document_id,
			document_name,
			document_is_auto_saved,
			document_is_saved,
			document_serialized_content,
		});
	}
}

// ============================================================================

#[wasm_bindgen(js_name = evaluateMathExpression)]
pub fn evaluate_math_expression(expression: &str) -> Option<f64> {
	// TODO: Rewrite our own purpose-built math expression parser that supports unit conversions.

	let mut context = meval::Context::new();
	context.var("tau", std::f64::consts::TAU);
	context.func("log", f64::log10);
	context.func("log10", f64::log10);
	context.func("log2", f64::log2);

	// Insert asterisks where implicit multiplication is used in the expression string
	let expression = implicit_multiplication_preprocess(expression);

	meval::eval_str_with_context(expression, &context).ok()
}

// Modified from this public domain snippet: <https://gist.github.com/Titaniumtown/c181be5d06505e003d8c4d1e372684ff>
// Discussion: <https://github.com/rekka/meval-rs/issues/28#issuecomment-1826381922>
pub fn implicit_multiplication_preprocess(expression: &str) -> String {
	let function = expression.to_lowercase().replace("log10(", "log(").replace("log2(", "logtwo(").replace("pi", "π").replace("tau", "τ");
	let valid_variables: Vec<char> = "eπτ".chars().collect();
	let letters: Vec<char> = ('a'..='z').chain('A'..='Z').collect();
	let numbers: Vec<char> = ('0'..='9').collect();
	let function_chars: Vec<char> = function.chars().collect();
	let mut output_string: String = String::new();
	let mut prev_chars: Vec<char> = Vec::new();

	for c in function_chars {
		let mut add_asterisk: bool = false;
		let prev_chars_len = prev_chars.len();

		let prev_prev_char = if prev_chars_len >= 2 { *prev_chars.get(prev_chars_len - 2).unwrap() } else { ' ' };

		let prev_char = if prev_chars_len >= 1 { *prev_chars.get(prev_chars_len - 1).unwrap() } else { ' ' };

		let c_letters_var = letters.contains(&c) | valid_variables.contains(&c);
		let prev_letters_var = valid_variables.contains(&prev_char) | letters.contains(&prev_char);

		if prev_char == ')' {
			if (c == '(') | numbers.contains(&c) | c_letters_var {
				add_asterisk = true;
			}
		} else if c == '(' {
			if (valid_variables.contains(&prev_char) | (')' == prev_char) | numbers.contains(&prev_char)) && !letters.contains(&prev_prev_char) {
				add_asterisk = true;
			}
		} else if numbers.contains(&prev_char) {
			if (c == '(') | c_letters_var {
				add_asterisk = true;
			}
		} else if letters.contains(&c) {
			if numbers.contains(&prev_char) | (valid_variables.contains(&prev_char) && valid_variables.contains(&c)) {
				add_asterisk = true;
			}
		} else if (numbers.contains(&c) | c_letters_var) && prev_letters_var {
			add_asterisk = true;
		}

		if add_asterisk {
			output_string += "*";
		}

		prev_chars.push(c);
		output_string += &c.to_string();
	}

	// We have to convert the Greek symbols back to ASCII because meval doesn't support unicode symbols as context constants
	output_string.replace("logtwo(", "log2(").replace('π', "pi").replace('τ', "tau")
}

/// Helper function for calling JS's `requestAnimationFrame` with the given closure
fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
	web_sys::window()
		.expect("No global `window` exists")
		.request_animation_frame(f.as_ref().unchecked_ref())
		.expect("Failed to call `requestAnimationFrame`");
}

/// Helper function for calling JS's `setTimeout` with the given closure and delay
fn set_timeout(f: &Closure<dyn FnMut()>, delay: Duration) {
	let delay = delay.clamp(Duration::ZERO, Duration::from_millis(i32::MAX as u64)).as_millis() as i32;
	web_sys::window()
		.expect("No global `window` exists")
		.set_timeout_with_callback_and_timeout_and_arguments_0(f.as_ref().unchecked_ref(), delay)
		.expect("Failed to call `setTimeout`");
}

/// Provides access to the `Editor` by calling the given closure with it as an argument.
fn editor<T: Default>(callback: impl FnOnce(&mut editor::application::Editor) -> T) -> T {
	EDITOR.with(|editor| {
		let mut guard = editor.try_lock();
		let Ok(Some(ref mut editor)) = guard.as_deref_mut() else { return T::default() };

		callback(editor)
	})
}

/// Provides access to the `Editor` and its `EditorHandle` by calling the given closure with them as arguments.
pub(crate) fn editor_and_handle(mut callback: impl FnMut(&mut Editor, &mut EditorHandle)) {
	EDITOR_HANDLE.with(|editor_handle| {
		editor(|editor| {
			let mut guard = editor_handle.try_lock();
			let Ok(Some(ref mut editor_handle)) = guard.as_deref_mut() else {
				log::error!("Failed to borrow editor handle");
				return;
			};

			// Call the closure with the editor and its handle
			callback(editor, editor_handle);
		})
	});
}

async fn poll_node_graph_evaluation() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	if !editor::node_graph_executor::run_node_graph().await {
		return;
	};

	editor_and_handle(|editor, handle| {
		let mut messages = VecDeque::new();
		if let Err(e) = editor.poll_node_graph_evaluation(&mut messages) {
			// TODO: This is a hacky way to suppress the error, but it shouldn't be generated in the first place
			if e != "No active document" {
				error!("Error evaluating node graph:\n{e}");
			}
		}

		// Clear the error display if there are no more errors
		if !messages.is_empty() {
			crate::NODE_GRAPH_ERROR_DISPLAYED.store(false, Ordering::SeqCst);
		}

		// Send each `FrontendMessage` to the JavaScript frontend
		for response in messages.into_iter().flat_map(|message| editor.handle_message(message)) {
			handle.send_frontend_message_to_js(response);
		}

		// If the editor cannot be borrowed then it has encountered a panic - we should just ignore new dispatches
	});
}

fn auto_save_all_documents() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	editor_and_handle(|editor, handle| {
		for message in editor.handle_message(PortfolioMessage::AutoSaveAllDocuments) {
			handle.send_frontend_message_to_js(message);
		}
	});
}

#[test]
fn implicit_multiplication_preprocess_tests() {
	assert_eq!(implicit_multiplication_preprocess("2pi"), "2*pi");
	assert_eq!(implicit_multiplication_preprocess("sin(2pi)"), "sin(2*pi)");
	assert_eq!(implicit_multiplication_preprocess("2sin(pi)"), "2*sin(pi)");
	assert_eq!(implicit_multiplication_preprocess("2sin(3(4 + 5))"), "2*sin(3*(4 + 5))");
	assert_eq!(implicit_multiplication_preprocess("3abs(-4)"), "3*abs(-4)");
	assert_eq!(implicit_multiplication_preprocess("-1(4)"), "-1*(4)");
	assert_eq!(implicit_multiplication_preprocess("(-1)4"), "(-1)*4");
	assert_eq!(implicit_multiplication_preprocess("(((-1)))(4)"), "(((-1)))*(4)");
	assert_eq!(implicit_multiplication_preprocess("2sin(pi) + 2cos(tau)"), "2*sin(pi) + 2*cos(tau)");
}
