//! The SVG filter primitives of the layer styles' finer options: the outline's paints.

use super::SvgFilterPrimitives;
use graphic_types::effects::{EffectCapture, GRADIENT_TABLE_SIZE, GlowFill};

/// Where an outline painted with something other than a solid color takes its bands' colors from, until they're first needed.
pub(super) enum OutlineBandColors {
	/// The markup of a drawn paint.
	Drawn(String),
	ContourGradient(GlowFill),
	/// The primitive holding the colors, once made.
	Made(String),
}

impl SvgFilterPrimitives {
	/// The primitive holding the colors that the outline effect at `index` paints its bands with, made the first time they're needed so a layer showing none of its bands makes none.
	pub(super) fn outline_band_colors(&mut self, colors: &mut OutlineBandColors, index: usize, region: [glam::DVec2; 2]) -> String {
		let made = match colors {
			OutlineBandColors::Made(made) => return made.clone(),
			OutlineBandColors::Drawn(markup) => self.paint_image(markup, region),
			OutlineBandColors::ContourGradient(gradient) => {
				let positions = self.map(EffectCapture { index, plane: 2 });
				self.contour_gradient_colors(&positions, gradient)
			}
		};
		*colors = OutlineBandColors::Made(made.clone());
		made
	}

	/// The colors of a contour `gradient` at each pixel's place across the outline, which the `positions` map holds in its alpha, looked up as the GPU does.
	pub(super) fn contour_gradient_colors(&mut self, positions: &str, gradient: &GlowFill) -> String {
		let Some((samples, opacity)) = gradient.gradient_samples(GRADIENT_TABLE_SIZE) else {
			return positions.to_string();
		};
		let table = |value: &dyn Fn(&[f32; 4]) -> f32| samples.iter().map(|sample| format!("{:.4}", value(sample).clamp(0., 1.))).collect::<Vec<_>>().join(" ");

		// The place moves into opaque color channels, which the tables look up the gradient's colors from in gamma
		let place = self.push("feColorMatrix", &format!(r#"in="{positions}" type="matrix" values="0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 0 1""#), "");
		let functions = ["R", "G", "B"]
			.into_iter()
			.enumerate()
			.map(|(channel, name)| format!(r#"<feFunc{name} type="table" tableValues="{}" />"#, table(&|sample| sample[channel])))
			.collect::<String>();
		let colors = self.push("feComponentTransfer", &format!(r#"in="{place}" color-interpolation-filters="sRGB""#), &functions);

		// A translucent gradient's alpha is looked up from the place where the map holds it, since premultiplied color channels lose precision at small places
		if opacity >= 1. && samples.iter().all(|sample| sample[3] >= 1.) {
			return colors;
		}
		let alpha_function = format!(r#"<feFuncA type="table" tableValues="{}" />"#, table(&|sample| sample[3] * opacity));
		let alpha = self.push("feComponentTransfer", &format!(r#"in="{positions}""#), &alpha_function);
		self.push("feComposite", &format!(r#"in="{colors}" in2="{alpha}" operator="in""#), "")
	}
}
