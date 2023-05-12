//! This file is where functions are defined to be called directly from JS.
//! It serves as a thin wrapper over the editor backend API that relies
//! on the dispatcher messaging system and more complex Rust data types.

use crate::helpers::{translate_key, Error};
use crate::{EDITOR_HAS_CRASHED, EDITOR_INSTANCES, JS_EDITOR_HANDLES};

use document_legacy::LayerId;
use editor::application::generate_uuid;
use editor::application::Editor;
use editor::consts::{FILE_SAVE_SUFFIX, GRAPHITE_DOCUMENT_VERSION};
use editor::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
use editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::messages::portfolio::utility_types::{ImaginateServerStatus, Platform};
use editor::messages::prelude::*;
use graph_craft::document::NodeId;
use graphene_core::raster::color::Color;

use serde::Serialize;
use serde_wasm_bindgen::{self, from_value};
use std::cell::RefCell;
use std::sync::atomic::Ordering;
use wasm_bindgen::prelude::*;

/// Set the random seed used by the editor by calling this from JS upon initialization.
/// This is necessary because WASM doesn't have a random number generator.
#[wasm_bindgen(js_name = setRandomSeed)]
pub fn set_random_seed(seed: u64) {
	editor::application::set_uuid_seed(seed);
}

/// We directly interface with the updateImage JS function for massively increased performance over serializing and deserializing.
/// This avoids creating a json with a list millions of numbers long.
#[wasm_bindgen(module = "/../src/wasm-communication/editor.ts")]
extern "C" {
	fn updateImage(path: Vec<u64>, nodeId: Option<u64>, mime: String, imageData: &[u8], transform: js_sys::Float64Array, document_id: u64);
	fn fetchImage(path: Vec<u64>, nodeId: Option<u64>, mime: String, document_id: u64, identifier: String);
	//fn dispatchTauri(message: String) -> String;
	fn dispatchTauri(message: String);
}

/// Provides a handle to access the raw WASM memory
#[wasm_bindgen(js_name = wasmMemory)]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
}

// To avoid wasm-bindgen from checking mutable reference issues using WasmRefCell we must make all methods take a non mutable reference to self.
// Not doing this creates an issue when rust calls into JS which calls back to rust in the same call stack.
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsEditorHandle {
	editor_id: u64,
	frontend_message_handler_callback: js_sys::Function,
}

fn window() -> web_sys::Window {
	web_sys::window().expect("no global `window` exists")
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
	window().request_idle_callback(f.as_ref().unchecked_ref()).unwrap();
	//window().request_animation_frame(f.as_ref().unchecked_ref()).expect("should register `requestAnimationFrame` OK");
}

// Sends a message to the dispatcher in the Editor Backend
async fn poll_node_graph_evaluation() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}
	editor::node_graph_executor::run_node_graph().await;

	// Get the editor instances, dispatch the message, and store the `FrontendMessage` queue response
	EDITOR_INSTANCES.with(|instances| {
		JS_EDITOR_HANDLES.with(|handles| {
			// Mutably borrow the editors, and if successful, we can access them in the closure
			instances.try_borrow_mut().map(|mut editors| {
				// Get the editor instance for this editor ID, then dispatch the message to the backend, and return its response `FrontendMessage` queue
				for (id, editor) in editors.iter_mut() {
					let handles = handles.borrow_mut();
					let handle = handles.get(id).unwrap();
					let mut messages = VecDeque::new();
					editor.poll_node_graph_evaluation(&mut messages);
					// Send each `FrontendMessage` to the JavaScript frontend

					let mut responses = Vec::new();
					for message in messages.into_iter() {
						responses.extend(editor.handle_message(message));
					}

					for response in responses.into_iter() {
						handle.send_frontend_message_to_js(response);
					}
					// If the editor cannot be borrowed then it has encountered a panic - we should just ignore new dispatches
				}
			})
		})
	});
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
impl JsEditorHandle {
	#[wasm_bindgen(constructor)]
	pub fn new(frontend_message_handler_callback: js_sys::Function) -> Self {
		let editor_id = generate_uuid();
		let editor = Editor::new();
		let editor_handle = JsEditorHandle {
			editor_id,
			frontend_message_handler_callback,
		};
		EDITOR_INSTANCES.with(|instances| instances.borrow_mut().insert(editor_id, editor));
		JS_EDITOR_HANDLES.with(|instances| instances.borrow_mut().insert(editor_id, editor_handle.clone()));
		editor_handle
	}

	// Sends a message to the dispatcher in the Editor Backend
	fn dispatch<T: Into<Message>>(&self, message: T) {
		// Process no further messages after a crash to avoid spamming the console
		if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
			return;
		}

		#[cfg(feature = "tauri")]
		{
			let message: Message = message.into();
			let message = ron::to_string(&message).unwrap();

			dispatchTauri(message);
		}
		#[cfg(not(feature = "tauri"))]
		{
			// Get the editor instances, dispatch the message, and store the `FrontendMessage` queue response
			let frontend_messages = EDITOR_INSTANCES.with(|instances| {
				// Mutably borrow the editors, and if successful, we can access them in the closure
				instances.try_borrow_mut().map(|mut editors| {
					// Get the editor instance for this editor ID, then dispatch the message to the backend, and return its response `FrontendMessage` queue
					editors
						.get_mut(&self.editor_id)
						.expect("EDITOR_INSTANCES does not contain the current editor_id")
						.handle_message(message.into())
				})
			});

			// Process any `FrontendMessage` responses resulting from the backend processing the dispatched message
			if let Ok(frontend_messages) = frontend_messages {
				// Send each `FrontendMessage` to the JavaScript frontend
				for message in frontend_messages.into_iter() {
					self.send_frontend_message_to_js(message);
				}
			}
		}
		// If the editor cannot be borrowed then it has encountered a panic - we should just ignore new dispatches
	}

	// Sends a FrontendMessage to JavaScript
	fn send_frontend_message_to_js(&self, mut message: FrontendMessage) {
		// Special case for update image data to avoid serialization times.
		if let FrontendMessage::UpdateImageData { document_id, image_data } = message {
			for image in image_data {
				#[cfg(not(feature = "tauri"))]
				{
					let transform = if let Some(transform_val) = image.transform {
						let transform = js_sys::Float64Array::new_with_length(6);
						transform.copy_from(&transform_val);
						transform
					} else {
						js_sys::Float64Array::default()
					};
					updateImage(image.path, image.node_id, image.mime, &image.image_data, transform, document_id);
				}
				#[cfg(feature = "tauri")]
				{
					let identifier = format!("http://localhost:3001/image/{:?}_{}", &image.path, document_id);
					fetchImage(image.path.clone(), image.node_id, image.mime, document_id, identifier);
				}
			}
			return;
		}
		if let FrontendMessage::UpdateDocumentLayerTreeStructure { data_buffer } = message {
			message = FrontendMessage::UpdateDocumentLayerTreeStructureJs { data_buffer: data_buffer.into() };
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
		let platform = match platform.as_str() {
			"Windows" => Platform::Windows,
			"Mac" => Platform::Mac,
			"Linux" => Platform::Linux,
			_ => Platform::Unknown,
		};

		self.dispatch(GlobalsMessage::SetPlatform { platform });
		self.dispatch(Message::Init);

		let f = std::rc::Rc::new(RefCell::new(None));
		let g = f.clone();

		*g.borrow_mut() = Some(Closure::new(move || {
			wasm_bindgen_futures::spawn_local(poll_node_graph_evaluation());

			// Schedule ourself for another requestAnimationFrame callback.
			request_animation_frame(f.borrow().as_ref().unwrap());
		}));

		request_animation_frame(g.borrow().as_ref().unwrap());
	}

	#[wasm_bindgen(js_name = tauriResponse)]
	pub fn tauri_response(&self, _message: JsValue) {
		#[cfg(feature = "tauri")]
		match ron::from_str::<Vec<FrontendMessage>>(&_message.as_string().unwrap()) {
			Ok(response) => {
				for message in response {
					self.send_frontend_message_to_js(message);
				}
			}
			Err(error) => {
				log::error!("tauri response: {:?}\n{:?}", error, _message);
			}
		}
	}

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

	/// Get the constant `GRAPHITE_DOCUMENT_VERSION`
	#[wasm_bindgen(js_name = graphiteDocumentVersion)]
	pub fn graphite_document_version(&self) -> String {
		GRAPHITE_DOCUMENT_VERSION.to_string()
	}

	/// Update layout of a given UI
	#[wasm_bindgen(js_name = updateLayout)]
	pub fn update_layout(&self, layout_target: JsValue, widget_id: u64, value: JsValue) -> Result<(), JsValue> {
		match (from_value(layout_target), from_value(value)) {
			(Ok(layout_target), Ok(value)) => {
				let message = LayoutMessage::UpdateLayout { layout_target, widget_id, value };
				self.dispatch(message);
				Ok(())
			}
			(target, val) => Err(Error::new(&format!("Could not update UI\nDetails:\nTarget: {:?}\nValue: {:?}", target, val)).into()),
		}
	}

	#[wasm_bindgen(js_name = loadPreferences)]
	pub fn load_preferences(&self, preferences: String) {
		let message = PreferencesMessage::Load { preferences };

		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = selectDocument)]
	pub fn select_document(&self, document_id: u64) {
		let message = PortfolioMessage::SelectDocument { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = newDocumentDialog)]
	pub fn new_document_dialog(&self) {
		let message = DialogMessage::RequestNewDocumentDialog;
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = documentOpen)]
	pub fn document_open(&self) {
		let message = PortfolioMessage::OpenDocument;
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
		let message = PortfolioMessage::AutoSaveDocument { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = closeDocumentWithConfirmation)]
	pub fn close_document_with_confirmation(&self, document_id: u64) {
		let message = PortfolioMessage::CloseDocumentWithConfirmation { document_id };
		self.dispatch(message);
	}

	#[wasm_bindgen(js_name = requestAboutGraphiteDialogWithLocalizedCommitDate)]
	pub fn request_about_graphite_dialog_with_localized_commit_date(&self, localized_commit_date: String) {
		let message = DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate { localized_commit_date };
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
	pub fn on_wheel_scroll(&self, x: f64, y: f64, mouse_keys: u8, wheel_delta_x: i32, wheel_delta_y: i32, wheel_delta_z: i32, modifiers: u8) {
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
	pub fn on_key_down(&self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		trace!("Key down {:?}, name: {}, modifiers: {:?}", key, name, modifiers);

		let message = InputPreprocessorMessage::KeyDown { key, modifier_keys };
		self.dispatch(message);
	}

	/// A keyboard button released
	#[wasm_bindgen(js_name = onKeyUp)]
	pub fn on_key_up(&self, name: String, modifiers: u8) {
		let key = translate_key(&name);
		let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");

		trace!("Key up {:?}, name: {}, modifiers: {:?}", key, name, modifier_keys);

		let message = InputPreprocessorMessage::KeyUp { key, modifier_keys };
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
	pub fn on_font_load(&self, font_family: String, font_style: String, preview_url: String, data: Vec<u8>, is_default: bool) -> Result<(), JsValue> {
		let message = PortfolioMessage::FontLoaded {
			font_family,
			font_style,
			preview_url,
			data,
			is_default,
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
	pub fn select_layer(&self, layer_path: Vec<LayerId>, ctrl: bool, shift: bool) {
		let message = DocumentMessage::SelectLayer { layer_path, ctrl, shift };
		self.dispatch(message);
	}

	/// Deselect all layers
	#[wasm_bindgen(js_name = deselectAllLayers)]
	pub fn deselect_all_layers(&self) {
		let message = DocumentMessage::DeselectAllLayers;
		self.dispatch(message);
	}

	/// Move a layer to be next to the specified neighbor
	#[wasm_bindgen(js_name = moveLayerInTree)]
	pub fn move_layer_in_tree(&self, folder_path: Vec<LayerId>, insert_index: isize) {
		let message = DocumentMessage::MoveSelectedLayersTo {
			folder_path,
			insert_index,
			reverse_index: true,
		};
		self.dispatch(message);
	}

	/// Set the name for the layer
	#[wasm_bindgen(js_name = setLayerName)]
	pub fn set_layer_name(&self, layer_path: Vec<LayerId>, name: String) {
		let message = DocumentMessage::SetLayerName { layer_path, name };
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	#[wasm_bindgen(js_name = translateCanvas)]
	pub fn translate_canvas(&self, delta_x: f64, delta_y: f64) {
		let message = NavigationMessage::TranslateCanvas { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Translates document (in viewport coords)
	#[wasm_bindgen(js_name = translateCanvasByFraction)]
	pub fn translate_canvas_by_fraction(&self, delta_x: f64, delta_y: f64) {
		let message = NavigationMessage::TranslateCanvasByViewportFraction { delta: (delta_x, delta_y).into() };
		self.dispatch(message);
	}

	/// Sends the blob URL generated by JS to the Image layer
	#[wasm_bindgen(js_name = setImageBlobURL)]
	pub fn set_image_blob_url(&self, document_id: u64, layer_path: Vec<LayerId>, node_id: Option<NodeId>, blob_url: String, width: f64, height: f64, transform: Option<js_sys::Float64Array>) {
		let resolution = (width, height);
		let message = PortfolioMessage::SetImageBlobUrl {
			document_id,
			layer_path: layer_path.clone(),
			node_id,
			blob_url,
			resolution,
		};
		self.dispatch(message);

		if let Some(array) = transform.filter(|array| array.length() == 6) {
			let mut transform: [f64; 6] = [0.; 6];
			array.copy_to(&mut transform);
			let message = document_legacy::Operation::SetLayerTransform { path: layer_path, transform };
			self.dispatch(message);
		}
	}

	/// Sends the blob URL generated by JS to the Imaginate layer in the respective document
	#[wasm_bindgen(js_name = setImaginateImageData)]
	pub fn set_imaginate_image_data(&self, document_id: u64, layer_path: Vec<LayerId>, node_path: Vec<NodeId>, image_data: Vec<u8>, width: u32, height: u32) {
		let message = PortfolioMessage::ImaginateSetImageData {
			document_id,
			node_path,
			layer_path,
			image_data,
			width,
			height,
		};
		self.dispatch(message);
	}

	/// Notifies the Imaginate layer of a new percentage of completion and whether or not it's currently generating
	#[wasm_bindgen(js_name = setImaginateGeneratingStatus)]
	pub fn set_imaginate_generating_status(&self, document_id: u64, layer_path: Vec<LayerId>, node_path: Vec<NodeId>, percent: Option<f64>, status: String) {
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
		self.dispatch(message);
	}

	/// Notifies the editor that the Imaginate server is available or unavailable
	#[wasm_bindgen(js_name = setImaginateServerStatus)]
	pub fn set_imaginate_server_status(&self, available: bool) {
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
		self.dispatch(message);
	}

	/// Sends the blob URL generated by JS to the Imaginate layer in the respective document
	#[wasm_bindgen(js_name = renderGraphUsingRasterizedRegionBelowLayer)]
	pub fn render_graph_using_rasterized_region_below_layer(
		&self,
		document_id: u64,
		layer_path: Vec<LayerId>,
		input_image_data: Vec<u8>,
		width: u32,
		height: u32,
		imaginate_node_path: Option<Vec<NodeId>>,
	) {
		let message = PortfolioMessage::RenderGraphUsingRasterizedRegionBelowLayer {
			document_id,
			layer_path,
			input_image_data,
			size: (width, height),
			imaginate_node_path,
		};
		self.dispatch(message);
	}

	/// Notifies the backend that the user connected a node's primary output to one of another node's inputs
	#[wasm_bindgen(js_name = connectNodesByLink)]
	pub fn connect_nodes_by_link(&self, output_node: u64, output_node_connector_index: usize, input_node: u64, input_node_connector_index: usize) {
		let message = NodeGraphMessage::ConnectNodesByLink {
			output_node,
			output_node_connector_index,
			input_node,
			input_node_connector_index,
		};
		self.dispatch(message);
	}

	/// Shifts the node and its children to stop nodes going on top of each other
	#[wasm_bindgen(js_name = shiftNode)]
	pub fn shift_node(&self, node_id: u64) {
		let message = NodeGraphMessage::ShiftNode { node_id };
		self.dispatch(message);
	}

	/// Notifies the backend that the user disconnected a node
	#[wasm_bindgen(js_name = disconnectNodes)]
	pub fn disconnect_nodes(&self, node_id: u64, input_index: usize) {
		let message = NodeGraphMessage::DisconnectNodes { node_id, input_index };
		self.dispatch(message);
	}

	/// Check for intersections between the curve and a rectangle defined by opposite corners
	#[wasm_bindgen(js_name = rectangleIntersects)]
	pub fn rectangle_intersects(&self, bezier_x: Vec<f64>, bezier_y: Vec<f64>, top: f64, left: f64, bottom: f64, right: f64) -> bool {
		let bezier = bezier_rs::Bezier::from_cubic_dvec2(
			(bezier_x[0], bezier_y[0]).into(),
			(bezier_x[1], bezier_y[1]).into(),
			(bezier_x[2], bezier_y[2]).into(),
			(bezier_x[3], bezier_y[3]).into(),
		);
		!bezier.rectangle_intersections((left, top).into(), (right, bottom).into()).is_empty() || bezier.is_contained_within((left, top).into(), (right, bottom).into())
	}

	/// Creates a new document node in the node graph
	#[wasm_bindgen(js_name = createNode)]
	pub fn create_node(&self, node_type: String, x: i32, y: i32) {
		let message = NodeGraphMessage::CreateNode { node_id: None, node_type, x, y };
		self.dispatch(message);
	}

	/// Notifies the backend that the user selected a node in the node graph
	#[wasm_bindgen(js_name = selectNodes)]
	pub fn select_nodes(&self, nodes: Vec<u64>) {
		let message = NodeGraphMessage::SelectNodes { nodes };
		self.dispatch(message);
	}

	/// Pastes the nodes based on serialized data
	#[wasm_bindgen(js_name = pasteSerializedNodes)]
	pub fn paste_serialized_nodes(&self, serialized_nodes: String) {
		let message = NodeGraphMessage::PasteNodes { serialized_nodes };
		self.dispatch(message);
	}

	/// Notifies the backend that the user double clicked a node
	#[wasm_bindgen(js_name = doubleClickNode)]
	pub fn double_click_node(&self, node: u64) {
		let message = NodeGraphMessage::DoubleClickNode { node };
		self.dispatch(message);
	}

	/// Notifies the backend that the selected nodes have been moved
	#[wasm_bindgen(js_name = moveSelectedNodes)]
	pub fn move_selected_nodes(&self, displacement_x: i32, displacement_y: i32) {
		let message = DocumentMessage::StartTransaction;
		self.dispatch(message);

		let message = NodeGraphMessage::MoveSelectedNodes { displacement_x, displacement_y };
		self.dispatch(message);
	}

	/// Toggle preview on node
	#[wasm_bindgen(js_name = togglePreview)]
	pub fn toggle_preview(&self, node_id: NodeId) {
		let message = NodeGraphMessage::TogglePreview { node_id };
		self.dispatch(message);
	}

	/// Pastes an image
	#[wasm_bindgen(js_name = pasteImage)]
	pub fn paste_image(&self, image_data: Vec<u8>, width: u32, height: u32, mouse_x: Option<f64>, mouse_y: Option<f64>) {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));
		let image = graphene_core::raster::Image::from_image_data(&image_data, width, height);
		let message = DocumentMessage::PasteImage { image, mouse };
		self.dispatch(message);
	}

	/// Toggle visibility of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerVisibility)]
	pub fn toggle_layer_visibility(&self, layer_path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerVisibility { layer_path };
		self.dispatch(message);
	}

	/// Toggle expansions state of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerExpansion)]
	pub fn toggle_layer_expansion(&self, layer_path: Vec<LayerId>) {
		let message = DocumentMessage::ToggleLayerExpansion { layer_path };
		self.dispatch(message);
	}

	/// Returns the string representation of the nodes contents
	#[wasm_bindgen(js_name = introspectNode)]
	pub fn introspect_node(&self, node_path: Vec<NodeId>) -> JsValue {
		let frontend_messages = EDITOR_INSTANCES.with(|instances| {
			// Mutably borrow the editors, and if successful, we can access them in the closure
			instances.try_borrow_mut().map(|mut editors| {
				// Get the editor instance for this editor ID, then dispatch the message to the backend, and return its response `FrontendMessage` queue
				let image = editors
					.get_mut(&self.editor_id)
					.expect("EDITOR_INSTANCES does not contain the current editor_id")
					.dispatcher
					.message_handlers
					.portfolio_message_handler
					.introspect_node(&node_path);
				let image = image?;
				let image = image.downcast_ref::<graphene_core::raster::ImageFrame<Color>>()?;
				let serializer = serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
				let message_data = image.serialize(&serializer).expect("Failed to serialize FrontendMessage");
				Some(message_data)
			})
		});
		frontend_messages.unwrap().unwrap_or_default()
	}
}

// Needed to make JsEditorHandle functions pub to Rust.
// The reason is not fully clear but it has to do with the #[wasm_bindgen] procedural macro.
impl JsEditorHandle {
	pub fn send_frontend_message_to_js_rust_proxy(&self, message: FrontendMessage) {
		self.send_frontend_message_to_js(message);
	}
}

impl Drop for JsEditorHandle {
	fn drop(&mut self) {
		// Consider removing after https://github.com/rustwasm/wasm-bindgen/pull/2984 is merged and released
		EDITOR_INSTANCES.with(|instances| instances.borrow_mut().remove(&self.editor_id));
	}
}
