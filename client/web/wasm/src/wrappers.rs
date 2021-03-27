use crate::shims::Error;
use graphite_editor_core::tools::{SelectAppendMode, ToolType};
use graphite_editor_core::Color as InnerColor;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Color(InnerColor);

#[wasm_bindgen]
impl Color {
	#[wasm_bindgen(constructor)]
	pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Color, JsValue> {
		match InnerColor::from_rgbaf32(red, green, blue, alpha) {
			Ok(v) => Ok(Self(v)),
			Err(e) => Err(Error::new(&e.to_string()).into()),
		}
	}
}

impl Color {
	pub fn inner(&self) -> InnerColor {
		self.0
	}
}

pub fn translate_tool(name: &str) -> Option<ToolType> {
	match name {
		"Select" => Some(ToolType::Select),
		"Crop" => Some(ToolType::Crop),
		"Navigate" => Some(ToolType::Navigate),
		"Sample" => Some(ToolType::Sample),
		"Path" => Some(ToolType::Path),
		"Pen" => Some(ToolType::Pen),
		"Line" => Some(ToolType::Line),
		"Rectangle" => Some(ToolType::Rectangle),
		"Ellipse" => Some(ToolType::Ellipse),
		"Shape" => Some(ToolType::Shape),
		_ => None,
	}
}

pub fn translate_append_mode(name: &str) -> Option<SelectAppendMode> {
	match name {
		"New" => Some(SelectAppendMode::New),
		"Add" => Some(SelectAppendMode::Add),
		"Subtract" => Some(SelectAppendMode::Subtract),
		"Intersect" => Some(SelectAppendMode::Intersect),
		_ => None,
	}
}
