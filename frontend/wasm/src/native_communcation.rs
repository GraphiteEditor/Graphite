use editor::messages::prelude::FrontendMessage;
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::prelude::*;

use crate::editor_api::{self, EditorHandle};

#[wasm_bindgen(js_name = "receiveNativeMessage")]
pub fn receive_native_message(buffer: ArrayBuffer) {
	let buffer = Uint8Array::new(buffer.as_ref()).to_vec();
	match ron::from_str::<Vec<FrontendMessage>>(str::from_utf8(buffer.as_slice()).unwrap()) {
		Ok(messages) => {
			let callback = move |handle: &mut EditorHandle| {
				for message in messages {
					handle.send_frontend_message_to_js_rust_proxy(message);
				}
			};
			editor_api::handle(callback);
		}
		Err(e) => log::error!("Failed to deserialize frontend messages: {e:?}"),
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
