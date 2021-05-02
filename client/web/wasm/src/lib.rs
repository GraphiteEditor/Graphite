pub mod document;
mod shims;
pub mod utils;
pub mod window;
pub mod wrappers;

use editor_core::{
	events::{DocumentResponse, Response, ToolResponse},
	Editor, LayerId,
};
use std::cell::RefCell;
use utils::WasmLog;
use wasm_bindgen::prelude::*;

// the thread_local macro provides a way to initialize static variables with non-constant functions
thread_local! { pub static EDITOR_STATE: RefCell<Editor> = RefCell::new(Editor::new(Box::new(handle_response))) }
static LOGGER: WasmLog = WasmLog;

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	log::set_logger(&LOGGER).expect("Failed to set logger");
	log::set_max_level(log::LevelFilter::Debug);
}

fn path_to_string(path: Vec<LayerId>) -> String {
	path.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(",")
}

fn handle_response(response: Response) {
	let response_type = response.to_string();
	match response {
		Response::Document(doc) => match doc {
			DocumentResponse::ExpandFolder { path, children } => {
				let children = children
					.iter()
					.map(|c| format!("name:{},visible:{},type:{}", c.name, c.visible, c.layer_type))
					.collect::<Vec<String>>()
					.join(";");
				send_response(response_type, &[path_to_string(path), children])
			}
			DocumentResponse::CollapseFolder { path } => send_response(response_type, &[path_to_string(path)]),
			DocumentResponse::DocumentChanged => log::error!("Wasm wrapper got request to update the document"),
		},
		Response::Tool(ToolResponse::UpdateCanvas { document }) => send_response(response_type, &[document]),
		Response::Tool(ToolResponse::SetActiveTool { tool_name }) => send_response(response_type, &[tool_name]),
	}
}
fn send_response(response_type: String, response_data: &[String]) {
	let data = response_data.iter().map(JsValue::from).collect();
	handleResponse(response_type, data);
}

#[wasm_bindgen(module = "/../src/response-handler.ts")]
extern "C" {
	fn handleResponse(responseType: String, responseData: Vec<JsValue>);
}

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
	format!("Hello, {}!", name)
}
