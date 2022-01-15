use wasm_bindgen::prelude::*;

/// The JavaScript `Error` type
#[wasm_bindgen]
extern "C" {
	#[derive(Clone, Debug)]
	pub type Error;

	#[wasm_bindgen(constructor)]
	pub fn new(msg: &str) -> Error;
}

/// Takes a string and matches it to its equivalently-named enum variant (useful for simple type translations)
macro_rules! match_string_to_enum {
	(match ($e:expr) {$($var:ident),* $(,)?}) => {
		match $e {
			$(
			stringify!($var) => Some($var),
			)*
			_ => None
		}
	};
}
pub(crate) use match_string_to_enum;
