//! Messaging with the native process. Serialized `FrontendMessage`s come in and are forwarded to JS,
//! `EditorCommand`s go back out. Encoding and decoding both live here so the wire format stays in one place.

use crate::editor_wrapper::EditorWrapper;
use crate::helpers::{render_image_data_to_canvases, wrapper};
use crate::wasm_value::WasmValue;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::prelude::*;

/// An `UpdateImageData` message as it crosses to the native frontend, with pixels in base64 since JSON would spell out each byte as a number.
#[derive(serde::Serialize, serde::Deserialize)]
struct EncodedImageData {
	images: Vec<EncodedImage>,
	canvas_ids_in_use: Vec<u64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EncodedImage {
	id: u64,
	width: u32,
	height: u32,
	pixels: String,
}

#[wasm_bindgen(js_name = "receiveNativeMessage")]
pub fn receive_native_message(buffer: ArrayBuffer) {
	let buffer = Uint8Array::new(buffer.as_ref()).to_vec();
	match serde_json::from_slice::<Vec<(String, String)>>(&buffer) {
		Ok(messages) => {
			wrapper(move |wrapper: &mut EditorWrapper| {
				for (name, data) in messages {
					match name.as_str() {
						"UpdateImageData" => match serde_json::from_str::<EncodedImageData>(&data) {
							Ok(encoded) => {
								let images: Vec<_> = encoded
									.images
									.iter()
									.filter_map(|image| {
										let pixels = BASE64.decode(&image.pixels).inspect_err(|e| error!("Failed to decode image {}: {e}", image.id)).ok()?;
										Some((image.id, image.width, image.height, pixels))
									})
									.collect();
								let images = images.iter().map(|(id, width, height, pixels)| (*id, *width, *height, pixels.as_slice()));
								render_image_data_to_canvases(images, &encoded.canvas_ids_in_use);
							}
							Err(e) => error!("Failed to deserialize image data: {e:?}"),
						},
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
				FrontendMessage::UpdateImageData { image_data, canvas_ids_in_use } => {
					let images = image_data
						.into_iter()
						.map(|image| EncodedImage {
							id: image.id,
							width: image.width,
							height: image.height,
							pixels: BASE64.encode(&image.pixels),
						})
						.collect();
					serde_json::to_string(&EncodedImageData { images, canvas_ids_in_use })
						.inspect_err(|e| log::error!("Failed to serialize {name} payload: {e}"))
						.ok()?
				}
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

#[cfg(all(test, feature = "editor"))]
mod tests {
	use super::*;
	use editor::messages::frontend::utility_types::RasterizedImage;
	use editor::messages::prelude::FrontendMessage;

	#[test]
	fn image_data_crosses_to_the_native_frontend_intact() {
		let image = RasterizedImage {
			id: 7,
			width: 2,
			height: 1,
			pixels: serde_bytes::ByteBuf::from(vec![1, 2, 3, 4, 5, 6, 7, 8]),
		};
		let message = FrontendMessage::UpdateImageData {
			image_data: vec![image],
			canvas_ids_in_use: vec![7, 9],
		};

		let encoded = encode_frontend_messages(vec![message]).unwrap();
		let messages: Vec<(String, String)> = serde_json::from_slice(&encoded).unwrap();
		let decoded: EncodedImageData = serde_json::from_str(&messages[0].1).unwrap();

		assert_eq!(messages[0].0, "UpdateImageData");
		assert_eq!(decoded.canvas_ids_in_use, [7, 9]);
		assert_eq!((decoded.images[0].id, decoded.images[0].width, decoded.images[0].height), (7, 2, 1));
		assert_eq!(BASE64.decode(&decoded.images[0].pixels).unwrap(), [1, 2, 3, 4, 5, 6, 7, 8]);
	}
}
