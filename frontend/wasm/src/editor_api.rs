#![allow(clippy::too_many_arguments)]
//
// This file is where functions are defined to be called directly from JS.
// It serves as a thin wrapper over the editor backend API that relies
// on the dispatcher messaging system and more complex Rust data types.
//
use crate::helpers::translate_key;
use crate::{Error, EDITOR_HAS_CRASHED, EDITOR_INSTANCES, JS_EDITOR_HANDLES};

use editor::application::generate_uuid;
use editor::application::Editor;
use editor::consts::FILE_SAVE_SUFFIX;
use editor::messages::input_mapper::utility_types::input_keyboard::ModifierKeys;
use editor::messages::input_mapper::utility_types::input_mouse::{EditorMouseState, ScrollDelta, ViewportBounds};
use editor::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
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
	//fn dispatchTauri(message: String) -> String;
	fn dispatchTauri(message: String);
}

/// Provides a handle to access the raw WASM memory
#[wasm_bindgen(js_name = wasmMemory)]
pub fn wasm_memory() -> JsValue {
	wasm_bindgen::memory()
}

/// Helper function for calling JS's `requestAnimationFrame` with the given closure
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
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

// ============================================================================

/// To avoid wasm-bindgen from checking mutable reference issues using WasmRefCell we must make all methods take a non-mutable reference to self.
/// Not doing this creates an issue when Rust calls into JS which calls back to Rust in the same call stack.
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsEditorHandle {
	editor_id: u64,
	frontend_message_handler_callback: js_sys::Function,
}

/// Provides access to the `Editor` instance and its `JsEditorHandle` by calling the given closure with them as arguments.
fn call_closure_with_editor_and_handle(mut f: impl FnMut(&mut Editor, &mut JsEditorHandle)) {
	EDITOR_INSTANCES.with(|instances| {
		JS_EDITOR_HANDLES.with(|handles| {
			instances
				.try_borrow_mut()
				.map(|mut editors| {
					for (id, editor) in editors.iter_mut() {
						let Ok(mut handles) = handles.try_borrow_mut() else {
							log::error!("Failed to borrow editor handles");
							continue;
						};
						let Some(js_editor) = handles.get_mut(id) else {
							log::error!("Editor ID ({id}) has no corresponding JsEditorHandle ID");
							continue;
						};

						// Call the closure with the editor and its handle
						f(editor, js_editor)
					}
				})
				.unwrap_or_else(|_| log::error!("Failed to borrow editor instances"));
		})
	});
}

async fn poll_node_graph_evaluation() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	editor::node_graph_executor::run_node_graph().await;

	call_closure_with_editor_and_handle(|editor, handle| {
		let mut messages = VecDeque::new();
		editor.poll_node_graph_evaluation(&mut messages);

		// Send each `FrontendMessage` to the JavaScript frontend
		for response in messages.into_iter().flat_map(|message| editor.handle_message(message)) {
			handle.send_frontend_message_to_js(response);
		}

		// If the editor cannot be borrowed then it has encountered a panic - we should just ignore new dispatches
	})
}

fn auto_save_all_documents() {
	// Process no further messages after a crash to avoid spamming the console
	if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
		return;
	}

	call_closure_with_editor_and_handle(|editor, handle| {
		for message in editor.handle_message(PortfolioMessage::AutoSaveAllDocuments) {
			handle.send_frontend_message_to_js(message);
		}
	});
}

// ============================================================================

#[wasm_bindgen]
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

			*g.borrow_mut() = Some(Closure::new(move || {
				wasm_bindgen_futures::spawn_local(poll_node_graph_evaluation());

				call_closure_with_editor_and_handle(|editor, handle| {
					for message in editor.handle_message(BroadcastMessage::TriggerEvent(BroadcastEvent::AnimationFrame)) {
						handle.send_frontend_message_to_js(message);
					}
				});

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
				log::error!("tauri response: {error:?}\n{_message:?}");
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
	/// If the insert index is `None`, it is inserted at the end of the folder (equivalent to index infinity).
	#[wasm_bindgen(js_name = moveLayerInTree)]
	pub fn move_layer_in_tree(&self, insert_parent_id: Option<u64>, insert_index: Option<usize>) {
		let insert_parent_id = insert_parent_id.map(NodeId);

		let parent = insert_parent_id.map(LayerNodeIdentifier::new_unchecked).unwrap_or_default();
		let message = DocumentMessage::MoveSelectedLayersTo {
			parent,
			insert_index: insert_index.map(|x| x as isize).unwrap_or(-1),
		};
		self.dispatch(message);
	}

	/// Set the name for the layer
	#[wasm_bindgen(js_name = setLayerName)]
	pub fn set_layer_name(&self, id: u64, name: String) {
		let id = NodeId(id);
		let message = NodeGraphMessage::SetName { node_id: id, name };
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

	/// Notifies the backend that the user connected a node's primary output to one of another node's inputs
	#[wasm_bindgen(js_name = connectNodesByLink)]
	pub fn connect_nodes_by_link(&self, output_node: u64, output_node_connector_index: usize, input_node: u64, input_node_connector_index: usize) {
		let output_node = NodeId(output_node);
		let input_node = NodeId(input_node);
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
		let node_id = NodeId(node_id);
		let message = NodeGraphMessage::ShiftNode { node_id };
		self.dispatch(message);
	}

	/// Notifies the backend that the user disconnected a node
	#[wasm_bindgen(js_name = disconnectNodes)]
	pub fn disconnect_nodes(&self, node_id: u64, input_index: usize) {
		let node_id = NodeId(node_id);
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
	pub fn create_node(&self, node_type: String, x: i32, y: i32) -> u64 {
		let id = NodeId(generate_uuid());
		let message = NodeGraphMessage::CreateNode { node_id: Some(id), node_type, x, y };
		self.dispatch(message);
		id.0
	}

	/// Notifies the backend that the user selected a node in the node graph
	#[wasm_bindgen(js_name = selectNodes)]
	pub fn select_nodes(&self, nodes: Vec<u64>) {
		let nodes = nodes.into_iter().map(NodeId).collect::<Vec<_>>();
		let message = NodeGraphMessage::SelectedNodesSet { nodes };
		self.dispatch(message);
	}

	/// Pastes the nodes based on serialized data
	#[wasm_bindgen(js_name = pasteSerializedNodes)]
	pub fn paste_serialized_nodes(&self, serialized_nodes: String) {
		let message = NodeGraphMessage::PasteNodes { serialized_nodes };
		self.dispatch(message);
	}

	/// Notifies the backend that the user double clicked a node
	#[wasm_bindgen(js_name = enterNestedNetwork)]
	pub fn enter_nested_network(&self, node: u64) {
		let node = NodeId(node);
		let message = NodeGraphMessage::EnterNestedNetwork { node };
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
	pub fn toggle_preview(&self, node_id: u64) {
		let node_id = NodeId(node_id);
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

	#[wasm_bindgen(js_name = pasteSvg)]
	pub fn paste_svg(&self, svg: String, mouse_x: Option<f64>, mouse_y: Option<f64>) {
		let mouse = mouse_x.and_then(|x| mouse_y.map(|y| (x, y)));
		let message = DocumentMessage::PasteSvg { svg, mouse };
		self.dispatch(message);
	}

	/// Toggle visibility of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerVisibility)]
	pub fn toggle_layer_visibility(&self, id: u64) {
		let id = NodeId(id);
		let message = NodeGraphMessage::ToggleHidden { node_id: id };
		self.dispatch(message);
	}

	/// Toggle expansions state of a layer from the layer list
	#[wasm_bindgen(js_name = toggleLayerExpansion)]
	pub fn toggle_layer_expansion(&self, id: u64) {
		let id = NodeId(id);
		let message = DocumentMessage::ToggleLayerExpansion { id };
		self.dispatch(message);
	}

	/// Returns the string representation of the nodes contents
	#[wasm_bindgen(js_name = introspectNode)]
	pub fn introspect_node(&self, node_path: Vec<u64>) -> JsValue {
		let node_path = node_path.into_iter().map(NodeId).collect::<Vec<_>>();
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

	#[wasm_bindgen(js_name = injectImaginatePollServerStatus)]
	pub fn inject_imaginate_poll_server_status(&self) {
		self.dispatch(PortfolioMessage::ImaginatePollServerStatus);
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
