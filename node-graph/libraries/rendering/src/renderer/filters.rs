//! The SVG filter primitives carrying out the filter effects like the GPU passes do.

use super::SvgFilterPrimitives;
use glam::DVec2;
use graphic_types::EffectOperation;
use graphic_types::effects::EffectCapture;
use graphic_types::filters::{Convolution, fill_shifts};
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

impl SvgFilterPrimitives {
	/// The image with a filter effect carried out on it, in a user space of one unit to a pixel of its effect space, or as it is for any other operation.
	/// Unless the filter runs `native`ly, which it does only where one user unit is one device pixel, it takes its result as the map the GPU renders for `capture`.
	pub(super) fn filter(&mut self, image: &str, operation: &EffectOperation, native: bool, capture: EffectCapture) -> String {
		let Some(convolution) = operation.convolution() else { return image.to_string() };

		match native.then(|| LatticeKernel::new(&convolution)).flatten() {
			Some(kernel) => {
				let convolved = self.convolve(image, &convolution, &kernel);
				self.uncover(&convolved, image)
			}
			None => self.uncovered_map(image, capture),
		}
	}

	/// The opaque colors of the map the GPU renders for `capture` given the image's coverage, matching the GPU's `uncover`, or the image as it is until the map resolves.
	/// Placing the map atop the image reads the image only once, so chains of filters taking maps stay linear in a browser that redoes a filter's input for each read.
	fn uncovered_map(&mut self, image: &str, capture: EffectCapture) -> String {
		let map = self.map(capture);
		self.push("feComposite", &format!(r#"in="{map}" in2="{image}" operator="atop""#), "")
	}

	/// Whether the filter natively reads its input more than a few times, so a chain of them in one filter would multiply their cost.
	pub(super) fn reads_input_often(&self, operation: &EffectOperation) -> bool {
		operation.convolution().is_some_and(|convolution| LatticeKernel::new(&convolution).is_some())
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

	/// The opaque colors of the image convolved by an `feConvolveMatrix` of the `kernel` over its visible colors, as the GPU's `convolve` describes.
	fn convolve(&mut self, image: &str, convolution: &Convolution, kernel: &LatticeKernel) -> String {
		let colors = self.visible_colors(image, convolution.reach());

		let ([columns, rows], [target_x, target_y]) = (kernel.order, kernel.target);
		let matrix = kernel.matrix.iter().map(f64::to_string).collect::<Vec<_>>().join(" ");
		let attributes = format!(
			r#"in="{colors}" order="{columns} {rows}" kernelMatrix="{matrix}" divisor="1" bias="{}" targetX="{target_x}" targetY="{target_y}" kernelUnitLength="1 1" edgeMode="none" preserveAlpha="true""#,
			convolution.offset
		);
		self.push("feConvolveMatrix", &attributes, "")
	}

	/// Filtered opaque straight `colors` given the image's coverage, matching the GPU's `uncover`.
	fn uncover(&mut self, colors: &str, image: &str) -> String {
		self.push("feComposite", &format!(r#"in="{colors}" in2="{image}" operator="in""#), "")
	}
}

#[cfg(test)]
mod tests {
	use super::super::{write_svg_effects_filter, write_svg_effects_filter_on_lattice};
	use core_types::blending::BlendMode;
	use glam::DVec2;
	use graphic_types::effect_layers::EffectLayers;
	use graphic_types::{EffectOperation, Effects};

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
}
