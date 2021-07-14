pub mod document;
mod shims;
pub mod utils;
pub mod window;
pub mod wrappers;

use editor_core::{message_prelude::*, Editor};
use js_sys::Array;
use js_sys::Uint8Array;
use std::cell::RefCell;
use utils::WasmLog;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

// the thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! {
	pub static EDITOR_STATE: RefCell<Editor> =  RefCell::new(Editor::new());
	pub static WEB_SOCKET: RefCell<WebSocketAdapter> = RefCell::new(WebSocketAdapter::new("wss://ws.graphite.kobert.dev"));

}
static LOGGER: WasmLog = WasmLog;

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

fn handle_responses(responses: Vec<FrontendMessage>) {
	for response in responses.into_iter() {
		handle_response(response)
	}
}

fn handle_response(response: FrontendMessage) {
	let response_type = response.to_discriminant().local_name();
	send_response(response_type, response);
}

fn send_response(response_type: String, response_data: FrontendMessage) {
	let response_data = JsValue::from_serde(&response_data).expect("Failed to serialize response");
	let _ = handleResponse(response_type, response_data).map_err(|error| log::error!("javascript threw an error: {:?}", error));
}

#[wasm_bindgen(module = "/../src/response-handler.ts")]
extern "C" {
	#[wasm_bindgen(catch)]
	fn handleResponse(responseType: String, responseData: JsValue) -> Result<(), JsValue>;
}

pub struct WebSocketAdapter {
	ws: WebSocket,
    queue: VecDeque<Message>,
}

impl WebSocketAdapter {
	/// Used to instantiate a Websocket connection
	/// # Examples
	/// ```should_panic
	/// let ws = crate::websocket::WebSocketAdapter::new("wss://echo.websocket.org").expect("Websocket creation failed");
	/// ```
	///
	/// # Errors
	/// Returns a JsValueError if the creation failed
	///
	pub fn new(url: &str) -> WebSocketAdapter {
		log::debug!("Websocket enry");

		// connect to the server
		//let ws = WebSocket::new(url).map_err(ClientError::WebSocketError)?;
		let ws = WebSocket::new_with_str(url, "Token-test").unwrap();

		// register the message callback
		let onmessage_callback = Closure::wrap(Box::new(WebSocketAdapter::message_callback) as Box<dyn FnMut(MessageEvent)>);
		ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
		// keep the closure alive, although it went out of scope
		onmessage_callback.forget();

		// register the error callback
		let onerror_callback = Closure::wrap(Box::new(WebSocketAdapter::error_callback) as Box<dyn FnMut(ErrorEvent)>);
		ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
		onerror_callback.forget();

		let cloned_ws = ws.clone();
		// register the open callback
		let onopen_callback = Closure::wrap(
			//Box::new(WebSocketAdapter::open_callback)
			Box::new(move |_| WebSocketAdapter::open_callback(&cloned_ws)) as Box<dyn FnMut(JsValue)>,
		);
		ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
		onopen_callback.forget();

		WebSocketAdapter { ws, queue: VecDeque::default() }
	}

	/// Close the WebSocket connention
	pub fn close(&self) {
		self.ws.close();
	}

	/// Sends a `&str` if the ws is in the ready state
	///
	/// # Errors
	/// Returns a WebSocketError if the connention is not ready or a different error occured
	///
	pub fn send_str(&self, message: &str) {
		match self.ws.ready_state() {
			1 => self.ws.send_with_str(message),
			_ => unreachable!(),
		}
		.unwrap()
	}

	pub fn send_message(&mut self, message: Message) {
        self.queue.push_back(message);
		if self.ws.ready_state() == 1 {
            for message in self.queue.drain(..) {
                self.ws.send_with_str(&serde_json::to_string(&message).unwrap());
            }
		}
	}

	/// Sends a `&mut [u8]` if the ws is in the ready state
	///
	/// # Errors
	/// Returns a WebSocketError if the connention is not ready or a different error occured
	///
	pub fn send_u8_arr(&self, message: &mut [u8]) {
		let view = unsafe { Uint8Array::view(message) };

		match self.ws.ready_state() {
			1 => self.ws.send_with_array_buffer_view(&view.slice(0, message.len() as u32)),
			_ => unreachable!(),
		};
	}

	fn message_callback(e: MessageEvent) {
		// handle message
		let data = e.data();
		if data.is_string() {
			let response = data.as_string().expect("Can't convert received data to a string");
			//log::debug!("message event, received data: {:?}", response);
			let parsed: Result<FrontendMessage, _> = serde_json::from_str(&response);
			if let Ok(message) = parsed {
                handle_response(message)
			} else {
				log::info!("Got message: {:?}", response);
			}
		} /*else {
			 let blob: web_sys::Blob = data.into();
			 let reader = FileReaderSync::new().unwrap();
			 let buff = reader.read_as_array_buffer(&blob).unwrap();
			 let u8_arr: js_sys::Uint8Array = js_sys::Uint8Array::new(&buff);
			 let size = u8_arr.length();
			 let mut res = vec![0u8; size as usize];

			 u8_arr.copy_to(&mut res);
			 log::debug!("arr: {:?}", res);
		 }*/
	}

	fn error_callback(e: ErrorEvent) {
		// handle error
		log::error!("error event: {:?}", e);
	}

	fn open_callback(cloned_ws: &WebSocket) {
		// handle open event
		log::debug!("socket opend");
		match cloned_ws.send_with_str("hallo") {
			Ok(_) => log::debug!("message delivered"),
			Err(err) => log::error!("error sending message: {:#?}", err),
		}
	}
}
