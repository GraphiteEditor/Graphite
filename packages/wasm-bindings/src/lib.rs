mod utils;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
	fn alert(s: &str);
}

#[wasm_bindgen(start)]
pub fn init() {
	utils::set_panic_hook();
	alert("Hello, Graphite!");
}
