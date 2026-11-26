//! The CPU steps of the layer styles' finer options: the outline's paints.

use crate::effect_kernels::{NEGLIGIBLE_LENGTH, Plane};
use graphic_types::effects::{GRADIENT_TABLE_SIZE, GlowFill, contour_gradient_position};

/// An outline's coverage outside the content's edge and inside it, where it reaches, and each pixel's place across it for a contour gradient.
pub(super) struct OutlineBands {
	pub outside: Option<Plane<f32>>,
	pub inside: Option<Plane<f32>>,
	pub positions: Option<Plane<f32>>,
}

/// The bands of an outline reaching `outside` pixels past the edge of the `silhouette` and `inside` pixels within it.
/// With a contour gradient's `start` and `span` in pixels (see `contour_gradient_span`), it also places each pixel across the outline.
/// Both bands grow by chamfer distances, which also measure each pixel's center from the edge for its place. O(width × height × log(weight)).
pub(super) fn outline_bands(silhouette: &Plane<f32>, outside: f64, inside: f64, contour_gradient: Option<(f64, f64)>) -> OutlineBands {
	let outward = (outside >= NEGLIGIBLE_LENGTH || contour_gradient.is_some()).then(|| silhouette.distances_to_silhouette(outside + 1.));
	let inward = (inside >= NEGLIGIBLE_LENGTH || contour_gradient.is_some()).then(|| silhouette.map(|alpha| 1. - alpha).distances_to_silhouette(inside + 1.));

	// The silhouette grown by the band's width, as `Plane::grown` grows it
	let band = |distances: &Option<Plane<f32>>, width: f64| {
		let reach = width as f32 + 1.;
		distances
			.as_ref()
			.filter(|_| width >= NEGLIGIBLE_LENGTH)
			.map(|distances| distances.map(|nearest| (reach - nearest).clamp(0., 1.)))
	};

	// Each pixel's center lies half a pixel short of its distance from the edge, inside or out
	let positions = match (&outward, &inward, contour_gradient) {
		(Some(outward), Some(inward), Some((start, span))) => Some(outward.zip(inward, |outward, inward| {
			let signed_distance = (outward - 0.5).max(0.) - (inward - 0.5).max(0.);
			contour_gradient_position(signed_distance, start as f32, span as f32)
		})),
		_ => None,
	};

	OutlineBands {
		outside: band(&outward, outside),
		inside: band(&inward, inside),
		positions,
	}
}

/// The premultiplied gamma-encoded color of a contour `gradient` at each pixel's place across the outline, from the same table of samples the other renderers read.
pub(super) fn contour_gradient_colors(gradient: &GlowFill, positions: &Plane<f32>) -> Plane<[f32; 4]> {
	let Some((samples, opacity)) = gradient.gradient_samples(GRADIENT_TABLE_SIZE) else {
		return positions.map(|_| [0.; 4]);
	};
	let last = GRADIENT_TABLE_SIZE - 1;

	positions.map(|position| {
		let [red, green, blue, alpha] = samples[((position * last as f32).round() as usize).min(last)];
		let alpha = alpha * opacity;
		[red * alpha, green * alpha, blue * alpha, alpha]
	})
}
