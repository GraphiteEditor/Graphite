//! The SVG filter primitives carrying out the filter effects like the GPU passes do.

use super::SvgFilterPrimitives;
use glam::DVec2;
use graphic_types::effects::EffectCapture;
use graphic_types::filters::{Convolution, Surroundings, fill_shifts};
use graphic_types::{EffectOperation, Effects, MinimumMaximumMode};
use raster_kernels::effect_kernels::{MAX_DIRECT_BOX_RADIUS, box_weight};
use std::collections::BTreeMap;

/// The most weights an `feConvolveMatrix` kernel holds before browsers carry it out with less precision.
const MAX_PRECISE_KERNEL_AREA: usize = 25;

/// A convolution's kernel on the user space's lattice, its weights listed as the matrix of an `feConvolveMatrix` of `order` weights across and down, with the `target` cell on the pixel being found.
struct LatticeKernel {
	order: [usize; 2],
	target: [usize; 2],
	matrix: Vec<f64>,
}

impl LatticeKernel {
	/// The kernel of `convolution`, its shifts in whole units of the user space, one to a pixel, or `None` if it holds more weights than browsers carry out precisely.
	fn new(convolution: &Convolution) -> Option<Self> {
		let mut weights: BTreeMap<(i64, i64), f64> = BTreeMap::new();
		for &(shift, weight) in &convolution.taps {
			*weights.entry((shift.x.round() as i64, shift.y.round() as i64)).or_default() += weight / convolution.divisor;
		}

		// The faint pixels' colors stack a copy for each shift within the square the kernel reaches across, which must stay small too
		let reach = weights.keys().map(|&(x, y)| x.abs().max(y.abs())).max().unwrap_or(0);
		let side = 2 * reach.unsigned_abs() as usize + 1;
		if side * side > MAX_PRECISE_KERNEL_AREA {
			return None;
		}

		// The matrix spans the weights and the pixel being found, and is applied turned halfway around, so it lists the weights from the last shift to the first
		let extent = |axis: fn(&(i64, i64)) -> i64| weights.keys().map(axis).fold((0, 0), |(least, most), value| (least.min(value), most.max(value)));
		let [(least_x, most_x), (least_y, most_y)] = [extent(|&(x, _)| x), extent(|&(_, y)| y)];
		let order = [(most_x - least_x + 1) as usize, (most_y - least_y + 1) as usize];
		let mut matrix = vec![0.; order[0] * order[1]];
		for (&(x, y), &weight) in &weights {
			let cell = (y - least_y) as usize * order[0] + (x - least_x) as usize;
			matrix[order[0] * order[1] - 1 - cell] = weight;
		}

		// The target is how far the kernel reads left of and above the pixel being found
		let target = [-least_x as usize, -least_y as usize];
		Some(Self { order, target, matrix })
	}
}

/// How much farther than the effects reach an SVG filter's region must extend, in document pixels, since a minimum's erosion of the colors reads past the region as black.
pub(super) fn svg_region_reach_beyond(effects: &Effects) -> f64 {
	effects
		.operations()
		.map(|operation| match operation {
			EffectOperation::MinimumMaximum {
				radius,
				mode: MinimumMaximumMode::Minimum,
			} => radius.max(0.).ceil(),
			_ => 0.,
		})
		.sum()
}

impl SvgFilterPrimitives {
	/// The image with a filter effect carried out on it, in a user space of one unit to a pixel of its effect space, or as it is for any other operation.
	/// A filter on the pixel lattice runs natively only where one user unit is one device pixel (`pixel_exact`), and otherwise takes its result as the map the GPU renders for `capture`.
	/// Where the fill `forked` the content into two planes, no filter reading its input many times runs natively, since the filter can't split to keep chains of them from multiplying.
	pub(super) fn filter(&mut self, image: &str, operation: &EffectOperation, pixel_exact: bool, forked: bool, capture: EffectCapture) -> String {
		let native = pixel_exact && !forked;
		if let Some(convolution) = operation.convolution() {
			return match native.then(|| LatticeKernel::new(&convolution)).flatten() {
				Some(kernel) => {
					let convolved = self.convolve(image, &convolution, &kernel);
					self.uncover(&convolved, image, convolution.surroundings)
				}
				None => self.uncovered_map(image, capture, convolution.surroundings),
			};
		}

		match *operation {
			// The Euclidean length of the gradient loses too much precision through the 8-bit results between primitives
			EffectOperation::FindEdges => self.uncovered_map(image, capture, Surroundings::Matted(1.)),
			EffectOperation::MinimumMaximum { .. } if forked => self.map_standing_in(capture, "feOffset", &format!(r#"in="{image}""#)),
			EffectOperation::MinimumMaximum { radius, mode } => self.neighborhood_extreme(image, radius, mode == MinimumMaximumMode::Maximum),
			_ => image.to_string(),
		}
	}

	/// The map the GPU renders for `capture` given the image's coverage, matching the GPU's `uncover`, or the image as it is until the map resolves.
	/// It reads the image only once, so chains of filters taking maps stay linear in a browser that redoes a filter's input for each read.
	/// Continued colors' maps are opaque and placed atop the image, while matted ones hold the whole result, since lifting off the gray isn't a composite.
	fn uncovered_map(&mut self, image: &str, capture: EffectCapture, surroundings: Surroundings) -> String {
		match surroundings {
			Surroundings::Continued => {
				let map = self.map(capture);
				self.push("feComposite", &format!(r#"in="{map}" in2="{image}" operator="atop""#), "")
			}
			Surroundings::Matted(_) => self.map_standing_in(capture, "feOffset", &format!(r#"in="{image}""#)),
		}
	}

	/// Whether the filter reads its input more than a few times (natively, for those on the pixel lattice), so a chain of them in one filter would multiply their cost.
	pub(super) fn reads_input_often(&self, operation: &EffectOperation, native: bool) -> bool {
		match operation.convolution() {
			Some(convolution) => native && LatticeKernel::new(&convolution).is_some(),
			None => matches!(operation, EffectOperation::MinimumMaximum { .. }),
		}
	}

	/// The image's straight colors as the filters read them, opaque where there's one, with faint pixels taking the nearest visible color within `reach` document pixels.
	/// Copies with the visible pixels made opaque and the rest left out stack nearest on top, so each pixel shows the first visible one exactly.
	fn visible_colors(&mut self, image: &str, reach: f64) -> String {
		// The alpha row raises 16 levels and up to full and leaves out 15 and below, matching the GPU's threshold, and beneath it all the covered pixels made opaque
		let opaque = |primitives: &mut Self, alpha_row: &str| {
			let values = format!("1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 {alpha_row}");
			primitives.push("feColorMatrix", &format!(r#"in="{image}" type="matrix" values="{values}""#), "")
		};
		let visible = opaque(self, "0 0 0 255 -15");
		let covered = opaque(self, "0 0 0 255 0");

		let mut layers = Vec::new();
		for [x, y] in fill_shifts(reach.ceil() as i32).into_iter().skip(1) {
			// Each copy moves against its shift, so the pixel it lands on reads the one the shift points to, written without negative zeros
			let shift = -DVec2::new(x as f64, y as f64) + DVec2::ZERO;
			layers.push(self.push("feOffset", &format!(r#"in="{visible}" dx="{}" dy="{}""#, shift.x, shift.y), ""));
		}
		let layers = [vec![covered], layers.into_iter().rev().collect(), vec![visible]].concat();

		let nodes: String = layers.iter().map(|layer| format!(r#"<feMergeNode in="{layer}" />"#)).collect();
		self.push("feMerge", "", &nodes)
	}

	/// The opaque colors of the image convolved by an `feConvolveMatrix` of the `kernel` over the colors its surroundings describe, as the GPU's `convolve` does.
	fn convolve(&mut self, image: &str, convolution: &Convolution, kernel: &LatticeKernel) -> String {
		let colors = match convolution.surroundings {
			Surroundings::Continued => self.visible_colors(image, convolution.reach()),
			Surroundings::Matted(gray) => self.over_gray(image, gray),
		};

		let ([columns, rows], [target_x, target_y]) = (kernel.order, kernel.target);
		let matrix = kernel.matrix.iter().map(f64::to_string).collect::<Vec<_>>().join(" ");
		let attributes = format!(
			r#"in="{colors}" order="{columns} {rows}" kernelMatrix="{matrix}" divisor="1" bias="{}" targetX="{target_x}" targetY="{target_y}" kernelUnitLength="1 1" edgeMode="none" preserveAlpha="true""#,
			convolution.offset
		);
		self.push("feConvolveMatrix", &attributes, "")
	}

	/// The image composited over an opaque gray of level `gray`.
	fn over_gray(&mut self, image: &str, gray: f64) -> String {
		let percent = gray * 100.;
		let flood = self.push("feFlood", &format!(r#"flood-color="rgb({percent}%, {percent}%, {percent}%)" flood-opacity="1""#), "");
		self.push("feComposite", &format!(r#"in="{image}" in2="{flood}" operator="over""#), "")
	}

	/// Filtered opaque `colors` given the image's coverage, as their `surroundings` describe, matching the GPU's `uncover`.
	fn uncover(&mut self, colors: &str, image: &str, surroundings: Surroundings) -> String {
		let Surroundings::Matted(gray) = surroundings else {
			return self.push("feComposite", &format!(r#"in="{colors}" in2="{image}" operator="in""#), "");
		};
		// The gray over the share the image leaves uncovered, taken off the opaque colors, which leaves the image's coverage as alpha and clamps as the GPU does
		let values = format!("0 0 0 0 {gray} 0 0 0 0 {gray} 0 0 0 0 {gray} 0 0 0 -1 1");
		let uncovered_gray = self.push("feColorMatrix", &format!(r#"in="{image}" type="matrix" values="{values}""#), "");
		self.push("feComposite", &format!(r#"in="{colors}" in2="{uncovered_gray}" operator="arithmetic" k2="1" k3="-1""#), "")
	}

	/// The image with each pixel's color the lightest (or with `maximum` false, the darkest) in the square of `radius` document pixels around it,
	/// and its coverage the opposite extreme, matching the GPU's `neighborhood_extreme`, by `feMorphology` of the opaque colors and of the coverage.
	fn neighborhood_extreme(&mut self, image: &str, radius: f64, maximum: bool) -> String {
		let radius = radius.max(0.);
		let (colors_operator, coverage_operator) = if maximum { ("dilate", "erode") } else { ("erode", "dilate") };

		let extreme_within = |primitives: &mut Self, whole_radius: f64| {
			if whole_radius <= 0. {
				return image.to_string();
			}
			let radius = format!(r#"radius="{whole_radius} {whole_radius}""#);

			// The maximum spreads every covered pixel's straight color made opaque, and the minimum the colors matted over white, lifted back off within the grown coverage
			let colors = match maximum {
				true => primitives.push("feColorMatrix", &format!(r#"in="{image}" type="matrix" values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 255 0""#), ""),
				false => primitives.over_gray(image, 1.),
			};
			let colors = primitives.push("feMorphology", &format!(r#"in="{colors}" operator="{colors_operator}" {radius}"#), "");
			let coverage = primitives.push("feMorphology", &format!(r#"in="{image}" operator="{coverage_operator}" {radius}"#), "");
			match maximum {
				true => primitives.push("feComposite", &format!(r#"in="{colors}" in2="{coverage}" operator="in""#), ""),
				false => primitives.uncover(&colors, &coverage, Surroundings::Matted(1.)),
			}
		};

		let near = extreme_within(self, radius.floor());
		let fraction = radius.fract();
		if fraction <= 0. {
			return near;
		}
		let far = extreme_within(self, radius.ceil());
		self.push("feComposite", &format!(r#"in="{near}" in2="{far}" operator="arithmetic" k2="{}" k3="{fraction}""#, 1. - fraction), "")
	}

	/// The image box blurred by a half width of `radius` user units in the color `space` ("sRGB" or "linearRGB"), weighing pixels as the GPU's box does (see [`box_weight`]).
	/// Where one user unit is one device pixel (`native`), it's a row then a column of `feConvolveMatrix` weights, and otherwise the GPU's result as the map for `capture`,
	/// with the Gaussian of the same variance standing in until the map resolves.
	pub(super) fn box_blur(&mut self, image: &str, radius: f64, space: &str, native: bool, capture: EffectCapture) -> String {
		// Past this the GPU halves the image before its box, which the map follows, and the kernels' markup stays short
		if !native || radius > MAX_DIRECT_BOX_RADIUS {
			let sigma = (radius * (radius + 1.) / 3.).sqrt();
			let attributes = format!(r#"in="{image}" stdDeviation="{sigma} {sigma}" color-interpolation-filters="{space}""#);
			return self.map_standing_in(capture, "feGaussianBlur", &attributes);
		}

		// The weights from one end to the other, which read the same turned around, summing to the divisor.
		// Unlike other kernels, they stay precise past `MAX_PRECISE_KERNEL_AREA`, since browsers quantize between a kernel's least and greatest weights, a box's only two.
		let reach = radius.ceil() as usize;
		let weights: Vec<f64> = (0..=2 * reach).map(|index| box_weight(radius, index.abs_diff(reach))).collect();
		let divisor: f64 = weights.iter().sum();
		let matrix = weights.iter().map(f64::to_string).collect::<Vec<_>>().join(" ");

		let mut pass = |input: &str, order: [usize; 2], [target_x, target_y]: [usize; 2]| {
			let attributes = format!(
				r#"in="{input}" order="{} {}" kernelMatrix="{matrix}" divisor="{divisor}" targetX="{target_x}" targetY="{target_y}" kernelUnitLength="1 1" edgeMode="none" preserveAlpha="false" color-interpolation-filters="{space}""#,
				order[0], order[1]
			);
			self.push("feConvolveMatrix", &attributes, "")
		};
		let row = pass(image, [weights.len(), 1], [reach, 0]);
		pass(&row, [1, weights.len()], [0, reach])
	}
}

#[cfg(test)]
mod tests {
	use super::super::{write_svg_effects_filter, write_svg_effects_filter_on_lattice};
	use core_types::blending::BlendMode;
	use glam::DVec2;
	use graphic_types::effect_layers::EffectLayers;
	use graphic_types::effects::EffectCapture;
	use graphic_types::{EffectOperation, Effects, MinimumMaximumMode};

	fn convolution_kernel(text: &str, scale: f64, offset: f64) -> EffectOperation {
		EffectOperation::convolution_kernel(text, Some(scale), offset)
	}

	/// The one layer that filters alone draw, with the content.
	fn one_layer(effects: &Effects) -> EffectLayers {
		EffectLayers::plan(effects, BlendMode::Normal, 1., true)
	}

	fn filter(operations: Vec<EffectOperation>, pixel_exact: bool) -> (String, usize) {
		let mut defs = String::new();
		let effects = Effects::from_operations(operations);
		let filter = write_svg_effects_filter_on_lattice(
			&mut defs,
			&effects,
			&one_layer(&effects),
			0,
			1.,
			1.,
			[DVec2::ZERO, DVec2::splat(10.)],
			0,
			&[],
			0..effects.len(),
			1.,
			pixel_exact,
		)
		.unwrap();
		(defs, filter.maps.len())
	}

	#[test]
	fn convolution_kernels_turn_their_kernels_around_over_the_visible_colors() {
		let (defs, maps) = filter(vec![convolution_kernel("0 0 0\n0 0 2\n0 0 0", 2., 51.)], true);
		assert_eq!(maps, 0);

		// The matrix spans the weight reading the pixel to the right and the pixel itself, turned around, after the stack of the nearest visible colors, nearest on top
		assert!(
			defs.contains(r#"<feColorMatrix in="SourceGraphic" type="matrix" values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 255 -15""#),
			"{defs}"
		);
		assert!(defs.contains(r#"<feMerge  result="effect-11"><feMergeNode in="effect-2" /><feMergeNode in="effect-10" />"#), "{defs}");
		assert!(defs.contains(r#"<feMergeNode in="effect-3" /><feMergeNode in="effect-1" /></feMerge>"#), "{defs}");
		assert!(defs.contains(r#"order="2 1" kernelMatrix="1 0" divisor="1" bias="0.2" targetX="0" targetY="0""#), "{defs}");
		assert!(defs.contains(r#"in2="SourceGraphic" operator="in""#), "{defs}");
		assert!(!defs.contains("-0\""), "{defs}");
	}

	#[test]
	fn convolution_kernels_target_their_center_in_any_shape() {
		// The order, the turned matrix, and the target of the `feConvolveMatrix` carrying out a kernel
		let matrix_of = |text: &str| {
			let (defs, _) = filter(vec![convolution_kernel(text, 1., 0.)], true);
			let attribute = |name: &str| defs.split(&format!(r#" {name}=""#)).nth(1).and_then(|rest| rest.split('"').next()).unwrap_or_default().to_string();
			["order", "kernelMatrix", "targetX", "targetY"].map(attribute)
		};

		// A row lists its weights from the last, centered on its middle, as a column does down, and an even count centers after its middle
		assert_eq!(matrix_of("1 2 3"), ["3 1", "3 2 1", "1", "0"]);
		assert_eq!(matrix_of("1\n2\n3"), ["1 3", "3 2 1", "0", "1"]);
		assert_eq!(matrix_of("1 2\n3 4"), ["2 2", "4 3 2 1", "1", "1"]);
	}

	#[test]
	fn convolution_kernels_take_the_gpus_map_off_the_pixel_lattice_or_past_precise_kernels() {
		// Where a user unit isn't a device pixel, or the kernel holds more weights than browsers keep precise, the colors come from the GPU, given the live coverage
		let (defs, maps) = filter(vec![convolution_kernel("0 0 0\n0 0 2\n0 0 0", 2., 51.)], false);
		assert_eq!(maps, 1);
		// The map lies atop the content, which shows as it is until the map resolves
		assert!(!defs.contains("feConvolveMatrix") && defs.contains(r#"in2="SourceGraphic" operator="atop""#), "{defs}");
		let (defs, maps) = filter(
			vec![convolution_kernel(
				&"1 1 1 1 1 1 1
"
				.repeat(7),
				49.,
				0.,
			)],
			true,
		);
		assert_eq!(maps, 1);
		assert!(!defs.contains("feConvolveMatrix"), "{defs}");

		// A fill forks the content into two, which take a map each
		let mut defs = String::new();
		let effects = Effects::from_operations([convolution_kernel("2", 1., 0.)]);
		let faded = write_svg_effects_filter(&mut defs, &effects, &one_layer(&effects), 0, 0.5, 1., [DVec2::ZERO, DVec2::ONE], 0, &[], 0..effects.len(), 1.).unwrap();
		assert_eq!(faded.maps.len(), 2);
	}

	fn box_blur(radius: f64, gamma: bool) -> EffectOperation {
		EffectOperation::Blur {
			radius_mask: Default::default(),
			radius,
			box_blur: true,
			gamma,
		}
	}

	/// The value of the attribute `name` of each `feConvolveMatrix` in the `defs`, in order.
	fn attribute_values(defs: &str, name: &str) -> Vec<String> {
		let primitives = defs.split("<feConvolveMatrix ").skip(1).filter_map(|rest| rest.split("/>").next());
		let value = |primitive: &str| primitive.split(&format!(r#"{name}=""#)).nth(1)?.split('"').next().map(str::to_string);
		primitives.filter_map(value).collect()
	}

	/// The filter of a run of `operations` drawing one layer at `fill`, with whether one user unit is one device pixel, and the captures of its maps.
	fn filter_at(operations: Vec<EffectOperation>, fill: f32, pixel_exact: bool) -> (String, Vec<EffectCapture>) {
		let mut defs = String::new();
		let effects = Effects::from_operations(operations);
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, fill, true);
		let filter = write_svg_effects_filter_on_lattice(&mut defs, &effects, &layers, 0, fill, 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[], 0..effects.len(), 1., pixel_exact).unwrap();
		(defs, filter.maps.iter().map(|map| map.capture).collect())
	}

	#[test]
	fn box_blurs_on_the_pixel_lattice_convolve_a_row_then_a_column_of_the_gpus_weights() {
		// A whole half width weighs its pixels evenly, a row centered on each pixel and then a column of the row's result, in linear light
		let (defs, maps) = filter_at(vec![box_blur(2., false)], 1., true);
		assert!(maps.is_empty(), "{defs}");
		assert_eq!(attribute_values(&defs, "order"), ["5 1", "1 5"]);
		assert_eq!(attribute_values(&defs, "kernelMatrix"), ["1 1 1 1 1"; 2]);
		assert_eq!(attribute_values(&defs, "divisor"), ["5"; 2]);
		assert_eq!([attribute_values(&defs, "targetX"), attribute_values(&defs, "targetY")], [["2", "0"], ["0", "2"]]);
		assert!(
			defs.contains(r#"<feConvolveMatrix in="SourceGraphic""#) && defs.contains(r#"<feConvolveMatrix in="effect-1""#),
			"{defs}"
		);
		assert_eq!(attribute_values(&defs, "color-interpolation-filters"), ["linearRGB"; 2]);
		assert!(defs.contains(r#"edgeMode="none" preserveAlpha="false""#), "{defs}");

		// A fractional half width weighs the pixels at its ends by the fraction of them it covers, here in gamma
		let (defs, _) = filter_at(vec![box_blur(2.5, true)], 1., true);
		assert_eq!(attribute_values(&defs, "order"), ["7 1", "1 7"]);
		assert_eq!(attribute_values(&defs, "kernelMatrix"), ["0.5 1 1 1 1 1 0.5"; 2]);
		assert_eq!(attribute_values(&defs, "divisor"), ["6"; 2]);
		assert_eq!(attribute_values(&defs, "color-interpolation-filters"), ["sRGB"; 2]);

		// Up to the widest box the GPU runs without halving the image, and on the faded fill as well as the content
		let (defs, maps) = filter_at(vec![box_blur(24., false)], 0.5, true);
		assert!(maps.is_empty(), "{defs}");
		assert_eq!(attribute_values(&defs, "order"), ["49 1", "1 49", "49 1", "1 49"]);
	}

	#[test]
	fn box_blurs_off_the_pixel_lattice_or_wider_than_the_gpus_direct_box_take_the_gpus_map() {
		// The GPU's result stands in place of the content's, with a Gaussian of the box's variance until it resolves
		let (defs, maps) = filter_at(vec![box_blur(2., false)], 1., false);
		assert_eq!(maps, [EffectCapture { index: 0, plane: 0 }]);
		assert!(!defs.contains("feConvolveMatrix"), "{defs}");
		assert!(
			defs.contains(r#"<!--effect-map-"#) && defs.contains(r#"<feGaussianBlur in="SourceGraphic" stdDeviation="1.4142135623730951 1.4142135623730951""#),
			"{defs}"
		);

		// A box the GPU blurs on a halved image is that box, and a fill forks the content into two maps
		let (defs, maps) = filter_at(vec![box_blur(24.5, true)], 0.5, true);
		assert_eq!(maps, [EffectCapture { index: 0, plane: 1 }, EffectCapture { index: 0, plane: 0 }]);
		assert!(!defs.contains("feConvolveMatrix"), "{defs}");
	}

	#[test]
	fn chained_native_convolutions_each_start_a_filter_of_their_own() {
		let blur = EffectOperation::Blur {
			radius_mask: Default::default(),
			radius: 2.,
			box_blur: false,
			gamma: false,
		};
		let sharpen = convolution_kernel("0 -1 0\n-1 5 -1\n0 -1 0", 1., 0.);
		let mut defs = String::new();
		let effects = Effects::from_operations([blur, sharpen.clone(), sharpen]);
		let filter = write_svg_effects_filter(&mut defs, &effects, &one_layer(&effects), 0, 1., 1., [DVec2::ZERO, DVec2::ONE], 0, &[], 0..effects.len(), 1.).unwrap();

		// Each convolution reads the result before it as the source of a filter later in the list
		assert_eq!(filter.ids.len(), 3);
		assert_eq!(defs.matches(r#"<feColorMatrix in="SourceGraphic""#).count(), 4, "{defs}");
	}

	/// How many primitives in the `defs` read the result read most.
	fn most_reads_of_a_result(defs: &str) -> usize {
		let results = defs.split(r#"result=""#).skip(1).filter_map(|rest| rest.split('"').next());
		let reads = |result: &str| defs.matches(&format!(r#"in="{result}""#)).count() + defs.matches(&format!(r#"in2="{result}""#)).count();
		results.map(reads).max().unwrap_or(0)
	}

	#[test]
	fn chained_convolutions_taking_maps_read_each_result_once() {
		// A browser works out a filter's input again for each read, so a chain reading any result twice would double its work at each stage
		let sharpen = || convolution_kernel("0 -1 0\n-1 5 -1\n0 -1 0", 1., 0.);
		for (fill, pixel_exact) in [(0.5, true), (1., false)] {
			let mut defs = String::new();
			let effects = Effects::from_operations([sharpen(), sharpen(), sharpen(), sharpen()]);
			let layers = EffectLayers::plan(&effects, BlendMode::Normal, fill, true);
			let filter = write_svg_effects_filter_on_lattice(&mut defs, &effects, &layers, 0, fill, 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[], 0..effects.len(), 1., pixel_exact).unwrap();
			assert!(!filter.maps.is_empty(), "{defs}");
			assert_eq!(most_reads_of_a_result(&defs), 1, "{defs}");
		}
	}

	#[test]
	fn chained_minimums_and_maximums_read_each_result_once_where_the_fill_forks_the_content() {
		// Natively each reads its input twice, or four times for a fractional radius, so where the fill keeps the filter from splitting they take the GPU's results
		let minimum_maximum = |radius, mode| EffectOperation::MinimumMaximum { radius, mode };
		let operations = [1.5, 1., 2.5].map(|radius| minimum_maximum(radius, MinimumMaximumMode::Minimum));
		let effects = Effects::from_operations(operations.into_iter().chain([minimum_maximum(1.5, MinimumMaximumMode::Maximum)]));
		let layers = EffectLayers::plan(&effects, BlendMode::Normal, 0.5, true);
		let mut defs = String::new();
		let filter = write_svg_effects_filter_on_lattice(&mut defs, &effects, &layers, 0, 0.5, 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[], 0..effects.len(), 1., true).unwrap();
		assert_eq!(filter.maps.len(), 8, "both planes of each take a map: {defs}");
		assert_eq!(most_reads_of_a_result(&defs), 1, "{defs}");
	}

	#[test]
	fn stylize_filters_matte_morph_and_map_where_svg_cant_reach() {
		// Emboss convolves the content over middle gray, then lifts the gray back off within the coverage in one step
		let (emboss, _) = filter(vec![EffectOperation::Emboss { angle: 0., height: 2., amount: 100. }], true);
		assert!(emboss.contains(r#"flood-color="rgb(50%, 50%, 50%)""#), "{emboss}");
		assert!(emboss.contains(r#"order="3 1" kernelMatrix="1 0 -1" divisor="1" bias="0.5" targetX="1" targetY="0""#), "{emboss}");
		assert!(emboss.contains(r#"values="0 0 0 0 0.5 0 0 0 0 0.5 0 0 0 0 0.5 0 0 0 -1 1""#), "{emboss}");
		assert!(emboss.contains(r#"operator="arithmetic" k2="1" k3="-1""#), "{emboss}");

		// Off the pixel lattice, its whole result comes from the GPU, with the content standing in as it is until the map resolves
		let (emboss, maps) = filter(vec![EffectOperation::Emboss { angle: 0., height: 2., amount: 100. }], false);
		assert_eq!(maps, 1);
		assert!(!emboss.contains("feConvolveMatrix") && emboss.contains(r#"--><feOffset in="SourceGraphic""#), "{emboss}");

		// Find Edges takes its whole result as a map too, for the fill and the content each
		let mut defs = String::new();
		let effects = Effects::from_operations([EffectOperation::FindEdges]);
		let find_edges = write_svg_effects_filter(&mut defs, &effects, &one_layer(&effects), 0, 0.5, 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[], 0..effects.len(), 1.).unwrap();
		let planes: Vec<usize> = find_edges.maps.iter().map(|map| map.capture.plane).collect();
		assert_eq!(planes, [1, 0], "the fill and the content each take their own");
		assert!(!defs.contains("-1 1\""), "nothing lifts white off the maps: {defs}");

		// A fractional radius mixes the morphologies of the whole radii around it
		let maximum = EffectOperation::MinimumMaximum {
			radius: 1.25,
			mode: MinimumMaximumMode::Maximum,
		};
		let (maximum, _) = filter(vec![maximum], true);
		assert!(
			maximum.contains(r#"operator="dilate" radius="1 1""#) && maximum.contains(r#"operator="erode" radius="2 2""#),
			"{maximum}"
		);
		assert!(maximum.contains(r#"operator="arithmetic" k2="0.75" k3="0.25""#), "{maximum}");

		// The maximum spreads every covered pixel's color, and the minimum lifts white back off its colors within the grown coverage, in a region grown for its erosion
		assert!(maximum.contains(r#"values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 255 0""#), "{maximum}");
		let mut defs = String::new();
		let effects = Effects::from_operations([EffectOperation::MinimumMaximum {
			radius: 2.,
			mode: MinimumMaximumMode::Minimum,
		}]);
		let minimum = write_svg_effects_filter(&mut defs, &effects, &one_layer(&effects), 0, 1., 1., [DVec2::ZERO, DVec2::splat(10.)], 0, &[], 0..effects.len(), 1.).unwrap();
		assert!(
			defs.contains(r#"operator="dilate" radius="2 2""#) && defs.contains(r#"values="0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 -1 1""#),
			"{defs}"
		);
		assert_eq!(minimum.region, [DVec2::splat(-5.), DVec2::splat(15.)]);
	}
}
