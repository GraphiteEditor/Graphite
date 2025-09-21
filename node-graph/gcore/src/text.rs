mod font_cache;
mod to_path;

use std::{
	borrow::Cow,
	collections::{HashMap, hash_map::Entry},
	fmt,
	sync::{Arc, Mutex},
};

use dyn_any::DynAny;
pub use font_cache::*;
use graphene_core_shaders::color::Color;
use parley::{Layout, StyleProperty};
use rustc_hash::FxBuildHasher;
use std::hash::BuildHasher;
use std::hash::{Hash, Hasher};
pub use to_path::*;

use crate::{consts::*, table::Table, vector::Vector};

/// Alignment of lines of type within a text block.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum TextAlign {
	#[default]
	Left,
	Center,
	Right,
	#[label("Justify")]
	JustifyLeft,
	// TODO: JustifyCenter, JustifyRight, JustifyAll
}

impl From<TextAlign> for parley::Alignment {
	fn from(val: TextAlign) -> Self {
		match val {
			TextAlign::Left => parley::Alignment::Left,
			TextAlign::Center => parley::Alignment::Middle,
			TextAlign::Right => parley::Alignment::Right,
			TextAlign::JustifyLeft => parley::Alignment::Justified,
		}
	}
}

#[derive(Clone, DynAny)]
pub struct Typography {
	pub layout: Layout<()>,
	pub font_family: String,
	pub color: Color,
	pub stroke: Option<(Color, f64)>,
}

impl fmt::Debug for Typography {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Typography")
			.field("font_family", &self.font_family)
			.field("color", &self.color)
			.field("stroke", &self.stroke)
			.finish()
	}
}

impl PartialEq for Typography {
	fn eq(&self, _other: &Self) -> bool {
		true
	}
}

impl Hash for Typography {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.layout.len().hash(state);
	}
}

impl Typography {
	pub fn to_vector(&self) -> Table<Vector> {
		// To implement this function, a clone of the `NewFontCacheWrapper` must be included in the typography data type
		Table::new()
	}
}

#[derive(Clone)]
pub struct NewFontCacheWrapper(pub Arc<Mutex<NewFontCache>>);

impl fmt::Debug for NewFontCacheWrapper {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("font cache").finish()
	}
}

impl PartialEq for NewFontCacheWrapper {
	fn eq(&self, _other: &Self) -> bool {
		log::error!("Font cache should not be compared");
		false
	}
}

unsafe impl dyn_any::StaticType for NewFontCacheWrapper {
	type Static = NewFontCacheWrapper;
}

pub struct NewFontCache {
	pub font_context: parley::FontContext,
	pub layout_context: parley::LayoutContext<()>,
	pub font_mapping: HashMap<Font, (String, parley::fontique::FontInfo)>,
	pub hash: u64,
}

impl NewFontCache {
	pub fn new() -> Self {
		let mut new = NewFontCache {
			font_context: parley::FontContext::new(),
			layout_context: parley::LayoutContext::new(),
			font_mapping: HashMap::new(),
			hash: 0,
		};

		let source_sans_font = Font::new(SOURCE_SANS_FONT_FAMILY.to_string(), SOURCE_SANS_FONT_STYLE.to_string());
		new.register_font(source_sans_font, SOURCE_SANS_FONT_DATA.to_vec());
		new
	}

	pub fn register_font(&mut self, font: Font, data: Vec<u8>) {
		match self.font_mapping.entry(font) {
			Entry::Occupied(occupied_entry) => {
				log::error!("Trying to register font that already is added: {:?}", occupied_entry.key());
			}
			Entry::Vacant(vacant_entry) => {
				let registered_font = self.font_context.collection.register_fonts(parley::fontique::Blob::from(data), None);
				if registered_font.len() > 1 {
					log::error!("Registered multiple fonts for {:?}. Only the first is accessible", vacant_entry.key());
				};
				match registered_font.into_iter().next() {
					Some((family_id, font_info)) => {
						let Some(family_name) = self.font_context.collection.family_name(family_id) else {
							log::error!("Could not get family name for font: {:?}", vacant_entry.key());
							return;
						};
						let Some(font_info) = font_info.into_iter().next() else {
							log::error!("Could not get font info for font: {:?}", vacant_entry.key());
							return;
						};
						// Hash the Font for a unique id and add it to the cached hash
						let hash_value = FxBuildHasher.hash_one(vacant_entry.key());
						self.hash = self.hash.wrapping_add(hash_value);

						vacant_entry.insert((family_name.to_string(), font_info));
					}
					None => log::error!("Could not register font for {:?}", vacant_entry.key()),
				}
			}
		}
	}

	pub fn generate_typography(&mut self, font: &Font, font_size: f32, text: &str) -> Option<Typography> {
		let Some((font_family, font_info)) = self.font_mapping.get(font) else {
			log::error!("Font not loaded: {:?}", font);
			return None;
		};
		let font_family = font_family.to_string();

		let mut builder = self.layout_context.ranged_builder(&mut self.font_context, text, 1., false);

		builder.push_default(StyleProperty::FontStack(parley::FontStack::Single(parley::FontFamily::Named(Cow::Owned(font_family.clone())))));
		builder.push_default(StyleProperty::FontSize(font_size));
		builder.push_default(StyleProperty::FontWeight(font_info.weight()));
		builder.push_default(StyleProperty::FontStyle(font_info.style()));
		builder.push_default(StyleProperty::FontWidth(font_info.width()));

		let mut layout: Layout<()> = builder.build(text);
		layout.break_all_lines(None);
		Some(Typography {
			layout,
			font_family,
			color: Color::BLACK,
			stroke: None,
		})
	}
}
