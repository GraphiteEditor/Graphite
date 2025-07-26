use crate::Message;
use editor::messages::prelude::*;
use serde::ser::Serialize;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

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
