//! The CPU steps of the layer styles' finer options: the outline's paints, and the bevel's contour and texture.

use crate::effect_kernels::{NEGLIGIBLE_LENGTH, Plane};
use core_types::Color;
use core_types::transfer_curve::{TransferCurve, TransferCurveEvaluator};
use glam::{DVec2, DVec3};
use graphic_types::effects::{
	ANTIALIASED_CONTOUR_OFFSETS, BEVEL_FOOT_FADE, BevelStyle, GRADIENT_TABLE_SIZE, GlowFill, bevel_lighting, bevel_lit, contour_end_slopes, contour_gradient_position, extended_contour,
	slope_contour_input, slope_contour_reshapes, texture_gray,
};
use raster_types::parallel_rows::fill_rows;

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

/// The gray of a bevel's texture at each pixel of a plane `width` by `height`, from its `colors` premultiplied in 8 bits as the GPU draws it.
pub(super) fn texture_grays(colors: Vec<Color>, width: usize, height: usize) -> Plane<f32> {
	let data = colors
		.into_iter()
		.map(|color| {
			let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
			texture_gray([red * alpha, green * alpha, blue * alpha, alpha].map(|channel| (channel * 255.).round() / 255.))
		})
		.collect();
	Plane { width, height, data }
}

/// What shapes a bevel's surface besides its height: a contour reshaping its slopes across a percentage of them, and a texture's gray sinking it by a percentage of its span.
pub(super) struct BevelSurface<'a> {
	pub style: BevelStyle,
	pub slope_contour: &'a TransferCurve,
	pub slope_range: f64,
	/// Whether the slope contour is sampled at nine points across each pixel.
	pub slope_antialiased: bool,
	pub texture: Option<Plane<f32>>,
	pub texture_depth: f64,
}

/// The offsets toward a pixel's right and lower neighbors at which an antialiased contour samples it, or just the pixel itself.
fn sample_offsets(antialiased: bool) -> Vec<[f64; 2]> {
	match antialiased {
		true => ANTIALIASED_CONTOUR_OFFSETS.iter().flat_map(|&y| ANTIALIASED_CONTOUR_OFFSETS.map(|x| [x, y])).collect(),
		false => vec![[0., 0.]],
	}
}

/// A plane's value at `offset` from the pixel at `x` and `y` toward its right and lower neighbors, interpolated between them and holding the edge pixels beyond the edges.
fn sample_between(plane: &Plane<f32>, x: isize, y: isize, [offset_x, offset_y]: [f64; 2]) -> f64 {
	let (columns, rows) = (plane.width as isize, plane.height as isize);
	let at = |x: isize, y: isize| plane.data[(y.clamp(0, rows - 1) * columns + x.clamp(0, columns - 1)) as usize] as f64;

	let top = at(x, y) * (1. - offset_x) + at(x + 1, y) * offset_x;
	let bottom = at(x, y + 1) * (1. - offset_x) + at(x + 1, y + 1) * offset_x;
	top * (1. - offset_y) + bottom * offset_y
}

/// How much each pixel of a bevel's surface is lit by the `light` past a flat surface's brightness (its highlight), and how much it falls short (its shadow), each from 0 to 1.
/// The surface's `height` runs from -0.5 at the slopes' foot to 0.5 at their top, which rise `rise` pixels as the `surface` options shape them, and it's lit through the `gloss` contour, antialiased if paired with true.
/// For PSD interop, a slope is the difference across a pixel's neighbors averaged over the three rows or columns they span, and antialiasing averages nine samples offset toward the right and lower neighbors.
/// O(width × height), nine times over when antialiased.
pub(super) fn bevel_shading(height: &Plane<f32>, surface: &BevelSurface, rise: f64, light: DVec3, gloss: Option<&(TransferCurveEvaluator, bool)>) -> [Plane<f32>; 2] {
	let (columns, rows) = (height.width as isize, height.height as isize);
	let reshaped = slope_contour_reshapes(surface.slope_contour, surface.style, surface.slope_range);
	let contour = surface.slope_contour.evaluator();
	let slopes = contour_end_slopes(&contour);
	let texture_depth = surface.texture_depth / 100.;

	// The surface's height in units of its span at an offset from a pixel, reshaped by the contour and sunk by the texture
	let surface_at = |x: isize, y: isize, offset: [f64; 2]| {
		let t = sample_between(height, x, y, offset) + 0.5;
		let shaped = match reshaped {
			true => extended_contour(&contour, slopes, slope_contour_input(t, surface.style, surface.slope_range)),
			false => t,
		};
		shaped - surface.texture.as_ref().map_or(0., |texture| texture_depth * sample_between(texture, x, y, offset))
	};

	// How much brighter than flat ground each pixel is lit, averaged over the antialiased contour's samples, each lighting a whole grid of the surface offset alike
	let offsets = sample_offsets(surface.slope_antialiased && reshaped);
	let mut brighter = Plane::<f32>::new(height.width, height.height);
	for &offset in &offsets {
		let mut shifted = Plane::<f32>::new(height.width, height.height);
		fill_rows(&mut shifted.data, height.width, |y, row| {
			row.iter_mut().enumerate().for_each(|(x, value)| *value = surface_at(x as isize, y as isize, offset) as f32)
		});

		let at = |x: isize, y: isize| shifted.data[(y.clamp(0, rows - 1) * columns + x.clamp(0, columns - 1)) as usize] as f64;
		fill_rows(&mut brighter.data, height.width, |y, row| {
			let y = y as isize;
			for (x, brighter) in row.iter_mut().enumerate() {
				let x = x as isize;
				let across = at(x + 1, y - 1) + at(x + 1, y) + at(x + 1, y + 1) - at(x - 1, y - 1) - at(x - 1, y) - at(x - 1, y + 1);
				let down = at(x - 1, y + 1) + at(x, y + 1) + at(x + 1, y + 1) - at(x - 1, y - 1) - at(x, y - 1) - at(x + 1, y - 1);
				*brighter += ((bevel_lit(DVec2::new(across, down) * rise / 6., light) - light.z) / offsets.len() as f64) as f32;
			}
		});
	}

	// The gloss contour reshapes how lit each pixel is, averaged likewise over samples toward its neighbors when antialiased
	let brighter = match gloss {
		Some((gloss, antialiased)) => {
			let offsets = sample_offsets(*antialiased);
			let mut glossy = Plane::<f32>::new(height.width, height.height);
			fill_rows(&mut glossy.data, height.width, |y, row| {
				for (x, value) in row.iter_mut().enumerate() {
					let lit = |offset| gloss.evaluate(sample_between(&brighter, x as isize, y as isize, offset) + light.z).clamp(0., 1.);
					*value = (offsets.iter().map(|&offset| lit(offset)).sum::<f64>() / offsets.len() as f64 - light.z) as f32;
				}
			});
			glossy
		}
		None => brighter,
	};

	let amounts = brighter.map(|brighter| {
		let (highlight, shadow) = bevel_lighting(brighter as f64, light.z);
		[highlight as f32, shadow as f32, 0., 0.]
	});
	[amounts.map(|[highlight, ..]| highlight), amounts.map(|[_, shadow, ..]| shadow)]
}

/// A bevel's highlight and shadow `amounts` beneath the content, faded out across the slopes' foot by their `height` (from -0.5 at the foot), as a gloss contour or texture needs.
/// For PSD interop, that keeps the flat ground they shade from reaching past the foot.
pub(super) fn foot_faded(amounts: [Plane<f32>; 2], height: &Plane<f32>) -> [Plane<f32>; 2] {
	amounts.map(|amount| amount.zip(height, |amount, height| amount.min((height + 0.5) / BEVEL_FOOT_FADE as f32)))
}
