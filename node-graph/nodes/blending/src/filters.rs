use core_types::list::Item;
use core_types::{Color, Ctx};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Effect, EffectOperation, Generator, Graphic, Vector, stamp_effect};
use vector_types::Gradient;

/// Recolors each pixel of the input graphics from the pixels around it, weighted by a kernel of numbers, for sharpening, blurring, edge-finding, and embossing filters of your own design.
#[node_macro::node(category("Raster: Filter"))]
fn convolution_kernel<T>(
	_: impl Ctx,
	/// The content to filter.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The weight of each pixel around the one being recolored, written as one row of the kernel per line, with the numbers in a row separated by spaces or commas. Shorter rows end in zeros, and numbers that can't be read count as zero.
	///
	/// The pixel being recolored lies at the middle row and column, or for even rows or columns, just right of or below the middle, so the kernel reaches one pixel further left or up. For kernels exceeding the 255×255 limit, only the 255 rows or columns nearest the center are kept.
	#[default("0 0 0 0 0\n0 0 -1 0 0\n0 -1 5 -1 0\n0 0 -1 0 0\n0 0 0 0 0")]
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	kernel: Item<String>,
	/// Whether the *Scale* is set by hand, rather than following the sum of the weights.
	#[widget(ParsedWidgetOverride::Hidden)]
	has_scale: Item<bool>,
	/// What the weighted sum is divided by. Otherwise, it's the sum of the weights so the brightness stays the same, or 1 if they sum to less, as edge-finding kernels do.
	#[default(1.)]
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	scale: Item<f64>,
	/// How much to brighten the result, in levels out of 255.
	#[range]
	#[soft(-255..255)]
	offset: Item<f64>,
) -> Item<T> {
	let scale = has_scale.element().then_some(*scale.element());

	let mut content = content;
	stamp_effect(&mut content, Effect::new(&EffectOperation::convolution_kernel(kernel.element(), scale, *offset.element())));
	content
}
