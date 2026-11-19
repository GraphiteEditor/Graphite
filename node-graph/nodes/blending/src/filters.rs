use core_types::list::Item;
use core_types::{Color, Ctx};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Adjustment, Effect, EffectOperation, Generator, Graphic, MinimumMaximumMode, Vector, stamp_effect};
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

/// Turns the input graphics gray and raises their edges in relief, as if lit from one side, with each color showing where it changes.
#[node_macro::node(category("Raster: Filter"))]
fn emboss<T>(
	_: impl Ctx,
	/// The content to emboss.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The direction the light comes from, counterclockwise from the right. An edge comes out light where the content is lighter on its side toward the light, so light shapes look pressed in and dark ones raised.
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(135.)]
	angle: Item<f64>,
	/// How far the relief reaches across each edge.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..10)]
	#[default(3.)]
	height: Item<f64>,
	/// How strongly the edges stand out from the gray.
	#[unit("%")]
	#[range]
	#[hard(0..)]
	#[soft(..500)]
	#[default(100.)]
	amount: Item<f64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Emboss {
			angle: *angle.element(),
			height: *height.element(),
			amount: *amount.element(),
		}),
	);
	content
}

/// Traces the edges of the input graphics in dark lines over white, darker where their colors change more steeply.
#[node_macro::node(category("Raster: Filter"))]
fn find_edges<T>(
	_: impl Ctx,
	/// The content to trace the edges of.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(&mut content, Effect::new(&EffectOperation::FindEdges));
	content
}

/// Inverts each color channel of the input graphics past its midpoint, so both its darkest and lightest values turn dark, like film exposed to light while developing.
#[node_macro::node(category("Raster: Filter"))]
fn solarize<T>(
	_: impl Ctx,
	/// The content to solarize.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(&mut content, Effect::new(&EffectOperation::Adjustment(Adjustment::Solarize)));
	content
}

/// Spreads the darkest or lightest colors of the input graphics, giving each pixel the darkest or lightest color in a square around it, separately for each channel.
#[node_macro::node(name("Minimum/Maximum"), category("Raster: Filter"))]
fn minimum_maximum<T>(
	_: impl Ctx,
	/// The content to spread the darkest or lightest colors of.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// Whether the darkest or the lightest colors spread.
	mode: Item<MinimumMaximumMode>,
	/// How far the square around each pixel reaches.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(1.)]
	radius: Item<f64>,
) -> Item<T> {
	let operation = EffectOperation::MinimumMaximum {
		radius: *radius.element(),
		mode: *mode.element(),
	};

	let mut content = content;
	stamp_effect(&mut content, Effect::new(&operation));
	content
}
