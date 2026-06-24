use graphic_types::raster_types::{Bitmap, CPU, Raster};
use visioncortex::PathSimplifyMode;
use vtracer::{ColorImage, ColorMode, Config, Hierarchical, SvgFile, convert};

pub fn convert_to_svg(image_data: &Raster<CPU>) -> SvgFile {
	let color_image = ColorImage {
		width: image_data.width() as usize,
		height: image_data.height() as usize,
		pixels: image_data.to_flat_u8().0,
	};
	let config: Config = Config {
		color_mode: ColorMode::Color,
		hierarchical: Hierarchical::Stacked,
		filter_speckle: 4,
		color_precision: 6,
		layer_difference: 16,
		mode: PathSimplifyMode::Spline,
		corner_threshold: 60,
		length_threshold: 4.,
		max_iterations: 10,
		splice_threshold: 45,
		path_precision: Some(6),
	};

	convert(color_image, config).expect("failed to obtain an SvgFile from vtracer.")
}
