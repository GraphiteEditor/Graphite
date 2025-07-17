use crate::vector::{VectorDataTable};
use graphene_core::Ctx;
pub use graphene_core::text::*;

#[node_macro::node(category(""))]
fn text<'i: 'n>(
	_: impl Ctx,
	font_cache: std::sync::Arc<FontCache>,
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
	/// Faux italic.
	#[unit("°")]
	#[default(0.)]
	tilt: f64,
	/// Splits each text glyph into its own instance, i.e. row in the table of vector data.
	#[default(false)]
	per_glyph_instances: bool,
) -> VectorDataTable {
	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		character_spacing,
		max_width,
		max_height,
		tilt,
	};

	let font_data = font_cache.get(&font_name).map(|f| load_font(f));

	to_path(&text, font_data, typesetting, per_glyph_instances)
}
