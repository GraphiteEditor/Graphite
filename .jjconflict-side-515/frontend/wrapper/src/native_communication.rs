//! Messaging with the native process. Serialized `FrontendMessage`s come in and are forwarded to JS,
//! `EditorCommand`s go back out. Encoding and decoding both live here so the wire format stays in one place.

use crate::editor_wrapper::{EditorWrapper, IMAGE_DATA_HASH};
use crate::helpers::{calculate_hash, render_image_data_to_canvases, wrapper};
use crate::wasm_value::WasmValue;
use js_sys::{ArrayBuffer, Uint8Array};
use std::sync::atomic::Ordering;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = "receiveNativeMessage")]
pub fn receive_native_message(buffer: ArrayBuffer) {
	let buffer = Uint8Array::new(buffer.as_ref()).to_vec();
	match serde_json::from_slice::<Vec<(String, String)>>(&buffer) {
		Ok(messages) => {
			wrapper(move |wrapper: &mut EditorWrapper| {
				for (name, data) in messages {
					match name.as_str() {
						"UpdateImageData" => {
							let new_hash = calculate_hash(&data);
							if IMAGE_DATA_HASH.swap(new_hash, Ordering::Relaxed) == new_hash {
								continue;
							}

							match serde_json::from_str::<Vec<RasterizedImage>>(&data) {
								Ok(image_data) => render_image_data_to_canvases(image_data.iter()),
								Err(e) => error!("Failed to deserialize image data: {e:?}"),
							}
						}
						_ => match serde_json::from_str::<WasmValue>(&data) {
							Ok(value) => wrapper.forward_serialized_frontend_message_to_js(&name, value),
							Err(e) => log::error!("Failed to deserialize frontend message {name}: {e:?}"),
						},
					}
				}
			});
		}
		Err(e) => log::error!("Failed to deserialize frontend messages: {e:?}"),
	}
}

#[cfg(feature = "editor")]
pub fn encode_frontend_messages(messages: Vec<editor::messages::prelude::FrontendMessage>) -> Option<Vec<u8>> {
	use editor::messages::prelude::FrontendMessage;
	use editor::utility_traits::{AsMessage, ToDiscriminant};

	let messages: Vec<(String, String)> = messages
		.into_iter()
		.map(|message| {
			let name = message.to_discriminant().local_name();
			let data = match message {
				FrontendMessage::UpdateImageData { image_data } => serde_json::to_string(&image_data).inspect_err(|e| log::error!("Failed to serialize {name} payload: {e}")).ok()?,
				message => crate::wasm_value::encode(&message)
					.inspect_err(|e| log::error!("Failed to serialize FrontendMessage {name}: {e}"))
					.ok()?,
			};
			Some((name, data))
		})
		.collect::<Option<_>>()?;

	serde_json::to_vec(&messages).ok()
}

#[cfg(all(feature = "editor", any(feature = "native", not(target_family = "wasm"))))]
pub fn decode_editor_command(data: &[u8]) -> Option<editor::messages::prelude::Message> {
	match serde_json::from_slice::<crate::EditorCommand>(data) {
		Ok(command) => Some(command.into()),
		Err(e) => {
			log::error!("Failed to deserialize editor command: {e}");
			None
		}
	}
}

pub fn initialize_native_communication() {
	let global = js_sys::global();

	// Get the function by name
	let func = js_sys::Reflect::get(&global, &JsValue::from_str("initializeNativeCommunication")).expect("Function not found");
	let func = func.dyn_into::<js_sys::Function>().expect("Not a function");

	// Call it
	func.call0(&JsValue::NULL).expect("Function call failed");
}

pub fn send_message_to_cef(message: String) {
	let global = js_sys::global();

	// Get the function by name
	let func = js_sys::Reflect::get(&global, &JsValue::from_str("sendNativeMessage")).expect("Function not found");

	let func = func.dyn_into::<js_sys::Function>().expect("Not a function");
	let array = Uint8Array::from(message.as_bytes());
	let buffer = array.buffer();

	// Call it with argument
	func.call1(&JsValue::NULL, &JsValue::from(buffer)).expect("Function call failed");
}

#[cfg(feature = "editor")]
pub(crate) use editor::messages::frontend::utility_types::RasterizedImage;

#[cfg(not(feature = "editor"))]
pub(crate) use RasterizedImageCopy as RasterizedImage;

#[cfg(any(not(feature = "editor"), test))]
#[derive(serde::Deserialize)]
pub(crate) struct RasterizedImageCopy {
	pub(crate) id: u64,
	pub(crate) width: u32,
	pub(crate) height: u32,
	pub(crate) pixels: Vec<u8>,
}

#[cfg(all(test, feature = "editor"))]
mod tests {
	use super::*;

	#[test]
	fn rasterized_image_copy_matches_editor_shape() {
		let editor_image = RasterizedImage {
			id: 7,
			width: 2,
			height: 1,
			pixels: serde_bytes::ByteBuf::from(vec![1, 2, 3, 4, 5, 6, 7, 8]),
		};
		let json = serde_json::to_string(&vec![editor_image]).unwrap();
		let decoded: Vec<RasterizedImageCopy> = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded[0].id, 7);
		assert_eq!((decoded[0].width, decoded[0].height), (2, 1));
		assert_eq!(decoded[0].pixels, [1, 2, 3, 4, 5, 6, 7, 8]);
	}
}
