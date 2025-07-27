use std::cell::RefCell;
use std::time::Duration;

use crate::Message;
use editor::messages::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

#[wasm_bindgen]
#[derive(Clone)]
pub struct EditorHandle;

#[wasm_bindgen]
impl EditorHandle {
	#[wasm_bindgen(constructor)]
	pub fn new(frontend_message_handler_callback: js_sys::Function) -> Self {
		EditorHandle
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
