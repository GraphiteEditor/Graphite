use crate::Message;
use editor::messages::prelude::*;
use graphene_std::Color;
use graphene_std::raster::Image;
use js_sys::{Object, Reflect};
use serde::ser::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, window};

// TODO: Remove
static IMAGE_DATA_HASH: AtomicU64 = AtomicU64::new(0);
fn calculate_hash<T: std::hash::Hash>(t: &T) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::Hasher;
	let mut hasher = DefaultHasher::new();
	t.hash(&mut hasher);
	hasher.finish()
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct EditorHandle {
	/// TODO: Remove
	/// We current do frontend message in native -> serde serialize -> json string -> serde deserialize -> frontend message in wasm -> JSValue -> browser
	/// We should do native -> V8Value -> browser.
	frontend_message_handler_callback: js_sys::Function,
}

#[wasm_bindgen]
impl EditorHandle {
	#[wasm_bindgen(constructor)]
	pub fn new(frontend_message_handler_callback: js_sys::Function) -> Self {
		EditorHandle { frontend_message_handler_callback }
	}

	/// TODO: Remove
	#[wasm_bindgen(js_name = sendMessageToFrontendFromCEF)]
	pub fn send_message_to_frontend_from_cef(&self, message: String) {
		let Ok(mut message) = serde_json::from_str::<FrontendMessage>(&message) else { return };

		if let FrontendMessage::UpdateImageData { ref image_data } = message {
			let new_hash = calculate_hash(image_data);
			let prev_hash = IMAGE_DATA_HASH.load(Ordering::Relaxed);

			if new_hash != prev_hash {
				render_image_data_to_canvases(image_data.as_slice());
				IMAGE_DATA_HASH.store(new_hash, Ordering::Relaxed);
			}
			return;
		}

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
}

impl EditorHandle {
	// Instead of dispatching the message to be run by the editor in wasm, we forward it to CEF, which transfers it to the editor running in the main native thread
	pub fn dispatch<T: Into<Message>>(&self, message: T) {
		send_message_to_cef(message)
	}
}

pub fn send_message_to_cef<T: Into<Message>>(message: T) {
	let message: Message = message.into();
	let Ok(serialized_message) = serde_json::to_string(&message) else {
		log::error!("Failed to serialize message");
		return;
	};

	let global = js_sys::global();

	// Get the function by name
	let func = js_sys::Reflect::get(&global, &JsValue::from_str("sendMessageToCef")).expect("Function not found");

	let func = func.dyn_into::<js_sys::Function>().expect("Not a function");

	// Call it with argument
	func.call1(&JsValue::NULL, &JsValue::from_str(&serialized_message)).expect("Function call failed");
}

// TODO: Remove
fn render_image_data_to_canvases(image_data: &[(u64, Image<Color>)]) {
	let window = match window() {
		Some(window) => window,
		None => {
			error!("Cannot render canvas: window object not found");
			return;
		}
	};
	let document = window.document().expect("window should have a document");
	let window_obj = Object::from(window);
	let image_canvases_key = JsValue::from_str("imageCanvases");

	let canvases_obj = match Reflect::get(&window_obj, &image_canvases_key) {
		Ok(obj) if !obj.is_undefined() && !obj.is_null() => obj,
		_ => {
			let new_obj = Object::new();
			if Reflect::set(&window_obj, &image_canvases_key, &new_obj).is_err() {
				error!("Failed to create and set imageCanvases object on window");
				return;
			}
			new_obj.into()
		}
	};
	let canvases_obj = Object::from(canvases_obj);

	for (placeholder_id, image) in image_data.iter() {
		let canvas_name = placeholder_id.to_string();
		let js_key = JsValue::from_str(&canvas_name);

		if Reflect::has(&canvases_obj, &js_key).unwrap_or(false) || image.width == 0 || image.height == 0 {
			continue;
		}

		let canvas: HtmlCanvasElement = document
			.create_element("canvas")
			.expect("Failed to create canvas element")
			.dyn_into::<HtmlCanvasElement>()
			.expect("Failed to cast element to HtmlCanvasElement");

		canvas.set_width(image.width);
		canvas.set_height(image.height);

		let context: CanvasRenderingContext2d = canvas
			.get_context("2d")
			.expect("Failed to get 2d context")
			.expect("2d context was not found")
			.dyn_into::<CanvasRenderingContext2d>()
			.expect("Failed to cast context to CanvasRenderingContext2d");
		let u8_data: Vec<u8> = image.data.iter().flat_map(|color| color.to_rgba8_srgb()).collect();
		let clamped_u8_data = wasm_bindgen::Clamped(&u8_data[..]);
		match ImageData::new_with_u8_clamped_array_and_sh(clamped_u8_data, image.width, image.height) {
			Ok(image_data_obj) => {
				if context.put_image_data(&image_data_obj, 0., 0.).is_err() {
					error!("Failed to put image data on canvas for id: {placeholder_id}");
				}
			}
			Err(e) => {
				error!("Failed to create ImageData for id: {placeholder_id}: {e:?}");
			}
		}

		let js_value = JsValue::from(canvas);

		if Reflect::set(&canvases_obj, &js_key, &js_value).is_err() {
			error!("Failed to set canvas '{canvas_name}' on imageCanvases object");
		}
	}
}
