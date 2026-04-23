//! Not immediately shader compatible due to needing [`GradientStops`] as a param, which needs [`Vec`]

use crate::adjust::Adjust;
use core_types::table::Table;
use core_types::{Color, Ctx};
use raster_types::{CPU, Raster};
use vector_types::GradientStops;

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27grdm%27%20%3D%20Gradient%20Map
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Gradient%20settings%20(Photoshop%206.0)
#[node_macro::node(category("Raster: Adjustment"))]
async fn gradient_map<T: Adjust<Color>, G: Into<Table<GradientStops>>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Raster<CPU>>,
		Table<Color>,
		Table<Color>,
		Table<GradientStops>,
		Table<GradientStops>,
	)]
	mut image: T,
	#[implementations(
		GradientStops,
		Table<GradientStops>,
		GradientStops,
		Table<GradientStops>,
		GradientStops,
		Table<GradientStops>,
	)]
	gradient: G,
	reverse: bool,
) -> T {
	let gradient: Table<GradientStops> = gradient.into();
	let Some(row) = gradient.get(0) else { return image };

	image.adjust(|color| {
		let intensity = color.luminance_srgb();
		let intensity = if reverse { 1. - intensity } else { intensity };
		row.element.evaluate(intensity as f64).to_linear_srgb()
	});

	image
}
