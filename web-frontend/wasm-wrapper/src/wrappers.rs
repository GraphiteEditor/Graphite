use graphite_editor::Color as InnerColor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Color(InnerColor);

#[wasm_bindgen]
impl Color {
	#[wasm_bindgen(constructor)]
	pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
		Self(InnerColor::from_rgbaf32(red, green, blue, alpha).unwrap_throw())
	}
}
