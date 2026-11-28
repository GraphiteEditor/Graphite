use core_types::list::Item;
use core_types::{Color, Ctx};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Adjustment, Effect, EffectOperation, Generator, Graphic, MinimumMaximumMode, NoiseDistribution, Vector, stamp_effect};
use vector_types::Gradient;

/// Recolors each pixel of the input graphics from the pixels around it, weighted by a kernel of numbers, for sharpening, blurring, edge-finding, and embossing filters of your own design.
#[node_macro::node(category("Raster: Filter"), version(2))]
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
	/// Whether the kernel also weighs the alpha, so edges sharpen, blur, or spread like the colors do, rather than keeping their transparency. The *Offset* brightens only the colors.
	apply_to_alpha: Item<bool>,
) -> Item<T> {
	let scale = has_scale.element().then_some(*scale.element());
	let operation = EffectOperation::convolution_kernel(kernel.element(), scale, *offset.element(), *apply_to_alpha.element());

	let mut content = content;
	stamp_effect(&mut content, Effect::new(&operation));
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

// TODO: For PSD interop, import the Sharpen and Sharpen More filters as 'Convolution Kernel' nodes with Apply to Alpha and the automatic Scale, since both kernels sum to 4.
// Sharpen's rows are `0 -1 0`, `-1 8 -1`, `0 -1 0`, and Sharpen More's `-1 -1 -1`, `-1 12 -1`, `-1 -1 -1`. Sharpen Edges is this node at 100% with Edges Only.
/// Sharpens the input graphics by exaggerating how much each pixel differs from a blur of the pixels around it, deepening the contrast along edges and details.
#[node_macro::node(category("Raster: Filter"), inspector("sharpen_inspector"))]
fn sharpen<T>(
	_: impl Ctx,
	/// The content to sharpen.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// How much each pixel's difference is exaggerated.
	#[unit("%")]
	#[range]
	#[hard(0..)]
	#[soft(..500)]
	#[default(50.)]
	amount: Item<f64>,
	/// How far the blur reaches, which sets how wide the details are that get sharpened.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(1.)]
	radius: Item<f64>,
	/// How much each exaggerated difference is softened toward none, in levels out of 255, so faint details like noise and grain stay as they are.
	#[range]
	#[hard(0..)]
	#[soft(..255)]
	threshold: Item<f64>,
	/// Whether only edges are sharpened, fading in from soft edges to full strength at strong ones, so smooth areas and noise stay as they are. Each pixel is compared against the four beside, above, and below it, in place of the blur.
	edges_only: Item<bool>,
) -> Item<T> {
	let operation = EffectOperation::Sharpen {
		amount: *amount.element(),
		radius: *radius.element(),
		threshold: *threshold.element(),
		edges_only: *edges_only.element(),
	};

	let mut content = content;
	stamp_effect(&mut content, Effect::new(&operation));
	content
}

/// Keeps only the fine details of the input graphics, how much each pixel differs from a blur of those around it, around middle gray where they're flat.
#[node_macro::node(category("Raster: Filter"))]
fn high_pass<T>(
	_: impl Ctx,
	/// The content to keep the details of.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// How far the blur reaches, so details narrower than this are kept.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(10.)]
	radius: Item<f64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(&mut content, Effect::new(&EffectOperation::HighPass { radius: *radius.element() }));
	content
}

/// Scatters random offsets across the colors of the input graphics, like film grain, fixed in place on the content.
#[node_macro::node(category("Raster: Filter"))]
fn add_noise<T>(
	_: impl Ctx,
	/// The content to add noise to.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// How strong the noise is, as a percentage of full brightness: the most a color shifts for uniform noise, or the standard deviation of its shifts for Gaussian noise.
	#[unit("%")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(12.5)]
	amount: Item<f64>,
	/// How the shifts are spread: evenly up to the amount, or clustered around none so most are subtle and a few are strong.
	distribution: Item<NoiseDistribution>,
	/// Whether each pixel's colors all shift together, so the noise changes only their brightness.
	monochromatic: Item<bool>,
	/// The random variation of the noise. The same seed always gives the same noise.
	#[hard(0..)]
	seed: Item<i64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::AddNoise {
			amount: *amount.element(),
			distribution: *distribution.element(),
			monochromatic: *monochromatic.element(),
			seed: *seed.element(),
		}),
	);
	content
}

/// Shows a random scattering of the pixels of the input graphics fully opaque and hides the rest, showing more where the content is more opaque, fixed in place on the content.
#[node_macro::node(category("Raster: Filter"))]
fn dissolve<T>(
	_: impl Ctx,
	/// The content to dissolve.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The percentage of pixels shown where the content is fully opaque.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(50.)]
	opacity: Item<f64>,
	/// The random variation of the scattering. The same seed always gives the same scattering.
	#[hard(0..)]
	seed: Item<i64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Dissolve {
			opacity: *opacity.element(),
			seed: *seed.element(),
		}),
	);
	content
}
