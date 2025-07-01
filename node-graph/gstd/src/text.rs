use crate::vector::{VectorData, VectorDataTable};
use glam::DAffine2;
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_core::Ctx;
pub use graphene_core::text::*;

#[node_macro::node(category(""))]
fn text<'i: 'n>(
	_: impl Ctx,
	editor: &'i WasmEditorApi,
	text: String,
	font_name: Font,
	#[unit(" px")]
	#[default(24.)]
	font_size: f64,
	#[unit("x")]
	#[default(1.2)]
	line_height_ratio: f64,
	#[unit(" px")]
	#[default(0.)]
	character_spacing: f64,
	#[unit(" px")]
	#[default(None)]
	max_width: Option<f64>,
	#[unit(" px")]
	#[default(None)]
	max_height: Option<f64>,
	#[unit("°")]
	#[default(0.)]
	tilt: f64,
) -> VectorDataTable {
	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		character_spacing,
		max_width,
		max_height,
		tilt,
	};

	let font_data = editor.font_cache.get(&font_name).map(|f| load_font(f));

	let glyph_vectors = to_path(&text, font_data, typesetting);

	let mut glyph_table = if glyph_vectors.is_empty() {
		VectorDataTable::new(VectorData::default())
	} else {
		VectorDataTable::default()
	};

	for glyph in glyph_vectors {
		let instance = graphene_core::instances::Instance {
			instance: glyph,
			transform: DAffine2::IDENTITY,
			..Default::default()
		};

		glyph_table.push(instance);
	}

	glyph_table
}
