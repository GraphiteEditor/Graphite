use core_types::Color;
use core_types::color::SRGBA8;
use core_types::list::{ATTR_GRADIENT_CYCLIC, ATTR_GRADIENT_HUE_DIRECTION, ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, ATTR_GRADIENT_SPREAD, ATTR_MIDPOINT, ATTR_POSITION, Item, List};
use core_types::render_complexity::RenderComplexity;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum GradientForm {
	/// Transitions the colors along a straight line.
	#[default]
	Linear,
	/// Transitions the colors outward from a center point.
	Radial,
}

/// A gradient's stops: a list of colors (linear, unassociated alpha) whose optional `position` and `midpoint`
/// attributes place each stop along the 0 to 1 range. Stops lacking the `position` attribute distribute evenly,
/// and stops lacking the `midpoint` attribute interpolate linearly (`0.5`).
#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
pub struct Gradient(pub List<Color>);

/// A gradient's per-stop parallel arrays, generic over color format: `GradientStops<Color>` nests inside the
/// [`GradientRamp`] exchange struct, while `GradientStops<SRGBA8>` is the JS-boundary shape used by the color picker UI.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, PartialEq, Default, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientStops<C> {
	pub color: Vec<C>,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub position: Option<Vec<f64>>,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub midpoint: Option<Vec<f64>>,
}

unsafe impl<C: dyn_any::StaticTypeSized> dyn_any::StaticType for GradientStops<C> {
	type Static = GradientStops<C::Static>;
}

impl From<&Gradient> for GradientStops<Color> {
	fn from(gradient: &Gradient) -> Self {
		Self {
			position: gradient.position_attribute(),
			midpoint: gradient.midpoint_attribute(),
			color: gradient.0.iter_element_values().copied().collect(),
		}
	}
}

impl From<&Gradient> for GradientStops<SRGBA8> {
	fn from(gradient: &Gradient) -> Self {
		Self {
			position: gradient.position_attribute(),
			midpoint: gradient.midpoint_attribute(),
			color: gradient.0.iter_element_values().map(|&color| SRGBA8::from(color)).collect(),
		}
	}
}

// The document path: faithful (no elision) so serialization stays a bijection under round-trip checks
impl From<GradientStops<Color>> for Gradient {
	fn from(stops: GradientStops<Color>) -> Self {
		let mut gradient = Gradient::from(stops.color);
		if let Some(position) = &stops.position {
			gradient.set_positions(position);
		}
		if let Some(midpoint) = &stops.midpoint {
			gradient.set_midpoints(midpoint);
		}
		gradient
	}
}

// Color picker round-trip: faithful, since eliding default-restating attributes needs the cyclic flag that only the ramp conversion holds
impl From<&GradientStops<SRGBA8>> for Gradient {
	fn from(stops: &GradientStops<SRGBA8>) -> Self {
		let mut gradient = Gradient::from(stops.color.iter().map(|&color| Color::from(color)).collect::<Vec<_>>());
		if let Some(position) = &stops.position {
			gradient.set_positions(position);
		}
		if let Some(midpoint) = &stops.midpoint {
			gradient.set_midpoints(midpoint);
		}
		gradient
	}
}

impl GradientStops<SRGBA8> {
	/// CSS `background-image` value drawing the stops as an SVG data URI, keeping straight-alpha interpolation.
	pub fn to_svg_background_image(&self, settings: GradientSettings) -> String {
		Gradient::from(self).to_svg_background_image(settings)
	}
}

/// The serialized exchange form of a gradient: its stops, with whole-ramp settings as sibling fields serialized
/// only when non-default. The space is the exception: it always serializes, so its absence marks a ramp
/// from before the field existed, which deserializes as the gamma those documents rendered with.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientRamp<C = Color> {
	pub stops: GradientStops<C>,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientSpread::is_default"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_spread: GradientSpread,
	// TODO: Elide the default again (removing `legacy_gamma`) when switching to the new document format and Ctrl-C node serialization format
	#[cfg_attr(feature = "serde", serde(default = "GradientSpace::legacy_gamma"))]
	pub gradient_space: GradientSpace,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_cyclic: bool,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientHueDirection::is_default"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_hue_direction: GradientHueDirection,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientInterpolation::is_default"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_interpolation: GradientInterpolation,
}

unsafe impl<C: dyn_any::StaticTypeSized> dyn_any::StaticType for GradientRamp<C> {
	type Static = GradientRamp<C::Static>;
}

impl<C> From<GradientStops<C>> for GradientRamp<C> {
	fn from(stops: GradientStops<C>) -> Self {
		Self {
			stops,
			gradient_spread: Default::default(),
			gradient_space: Default::default(),
			gradient_cyclic: Default::default(),
			gradient_hue_direction: Default::default(),
			gradient_interpolation: Default::default(),
		}
	}
}

impl From<&Gradient> for GradientRamp {
	fn from(gradient: &Gradient) -> Self {
		Self {
			stops: gradient.into(),
			gradient_spread: Default::default(),
			gradient_space: Default::default(),
			gradient_cyclic: Default::default(),
			gradient_hue_direction: Default::default(),
			gradient_interpolation: Default::default(),
		}
	}
}

impl From<Gradient> for GradientRamp {
	fn from(gradient: Gradient) -> Self {
		Self::from(&gradient)
	}
}

impl From<GradientRamp> for Gradient {
	fn from(ramp: GradientRamp) -> Self {
		Gradient::from(ramp.stops)
	}
}

impl From<&GradientRamp> for Gradient {
	fn from(ramp: &GradientRamp) -> Self {
		Gradient::from(ramp.stops.clone())
	}
}

// The runtime wire form: whole-ramp settings ride as the gradient item's attributes in its containing list,
// where the Fill kernel, chain setter nodes, and renderers read and write them
impl From<GradientRamp> for Item<Gradient> {
	fn from(ramp: GradientRamp) -> Self {
		let mut item = Item::new_from_element(Gradient::from(ramp.stops));
		if !ramp.gradient_spread.is_default() {
			item.set_attribute(ATTR_GRADIENT_SPREAD, ramp.gradient_spread);
		}
		if !ramp.gradient_space.is_default() {
			item.set_attribute(ATTR_GRADIENT_SPACE, ramp.gradient_space);
		}
		if ramp.gradient_cyclic {
			item.set_attribute(ATTR_GRADIENT_CYCLIC, ramp.gradient_cyclic);
		}
		if !ramp.gradient_hue_direction.is_default() {
			item.set_attribute(ATTR_GRADIENT_HUE_DIRECTION, ramp.gradient_hue_direction);
		}
		if !ramp.gradient_interpolation.is_default() {
			item.set_attribute(ATTR_GRADIENT_INTERPOLATION, ramp.gradient_interpolation);
		}
		item
	}
}

impl From<&Item<Gradient>> for GradientRamp {
	fn from(item: &Item<Gradient>) -> Self {
		Self {
			stops: item.element().into(),
			gradient_spread: item.attribute_cloned_or_default(ATTR_GRADIENT_SPREAD),
			gradient_space: item.attribute_cloned_or_default(ATTR_GRADIENT_SPACE),
			gradient_cyclic: item.attribute_cloned_or_default(ATTR_GRADIENT_CYCLIC),
			gradient_hue_direction: item.attribute_cloned_or_default(ATTR_GRADIENT_HUE_DIRECTION),
			gradient_interpolation: item.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION),
		}
	}
}

impl From<&GradientRamp> for GradientStops<SRGBA8> {
	fn from(ramp: &GradientRamp) -> Self {
		Self {
			position: ramp.stops.position.clone(),
			midpoint: ramp.stops.midpoint.clone(),
			color: ramp.stops.color.iter().map(|&color| SRGBA8::from(color)).collect(),
		}
	}
}

impl From<&GradientStops<SRGBA8>> for GradientRamp {
	fn from(stops: &GradientStops<SRGBA8>) -> Self {
		Self::from(Gradient::from(stops))
	}
}

impl From<&GradientRamp> for GradientRamp<SRGBA8> {
	fn from(ramp: &GradientRamp) -> Self {
		Self {
			stops: ramp.into(),
			gradient_spread: ramp.gradient_spread,
			gradient_space: ramp.gradient_space,
			gradient_cyclic: ramp.gradient_cyclic,
			gradient_hue_direction: ramp.gradient_hue_direction,
			gradient_interpolation: ramp.gradient_interpolation,
		}
	}
}

impl From<&Gradient> for GradientRamp<SRGBA8> {
	fn from(gradient: &Gradient) -> Self {
		Self {
			stops: gradient.into(),
			gradient_spread: Default::default(),
			gradient_space: Default::default(),
			gradient_cyclic: Default::default(),
			gradient_hue_direction: Default::default(),
			gradient_interpolation: Default::default(),
		}
	}
}

// Color picker round-trip: the picker sends every position explicitly, so elide the ones restating the even distribution the cyclic flag selects
impl From<&GradientRamp<SRGBA8>> for GradientRamp {
	fn from(ramp: &GradientRamp<SRGBA8>) -> Self {
		let mut gradient = Gradient::from(&ramp.stops);
		gradient.elide_default_attributes(ramp.gradient_cyclic);

		Self {
			gradient_spread: ramp.gradient_spread,
			gradient_space: ramp.gradient_space,
			gradient_cyclic: ramp.gradient_cyclic,
			gradient_hue_direction: ramp.gradient_hue_direction,
			gradient_interpolation: ramp.gradient_interpolation,
			..Self::from(gradient)
		}
	}
}

impl<C> GradientRamp<C> {
	/// Overwrites the whole-ramp settings fields as one bundle. This writes the cyclic flag without holding the stops in place
	/// (unlike [`GradientRamp::with_cyclic`]), so they must already be authored under `settings.cyclic`.
	pub fn with_settings(mut self, settings: GradientSettings) -> Self {
		let GradientSettings {
			spread,
			cyclic,
			space,
			hue_direction,
			interpolation,
		} = settings;

		self.gradient_spread = spread;
		self.gradient_cyclic = cyclic;
		self.gradient_space = space;
		self.gradient_hue_direction = hue_direction;
		self.gradient_interpolation = interpolation;

		self
	}
}

impl GradientRamp {
	pub fn black_to_white() -> Self {
		Self::from(Gradient::black_to_white())
	}

	/// Sets the cyclic flag, holding the stops at the positions they already occupy.
	/// The ramp owns this rather than leaving it to the runtime type's elision because `cyclic` determines different default elided positions.
	pub fn with_cyclic(mut self, gradient_cyclic: bool) -> Self {
		if self.gradient_cyclic == gradient_cyclic {
			return self;
		}

		let mut gradient = Gradient::from(self.stops);
		gradient.hold_positions_across_cyclic_change(self.gradient_cyclic, gradient_cyclic);

		self.stops = (&gradient).into();
		self.gradient_cyclic = gradient_cyclic;
		self
	}
}

impl From<List<Color>> for Gradient {
	fn from(colors: List<Color>) -> Self {
		Self(colors)
	}
}

impl From<Vec<Color>> for Gradient {
	fn from(colors: Vec<Color>) -> Self {
		Self(colors.into_iter().map(Item::new_from_element).collect())
	}
}

impl RenderComplexity for Gradient {
	fn render_complexity(&self) -> usize {
		1
	}
}

/// Controls accuracy vs. number of samples tradeoff. 2/255 means the linear approximation will
/// deviate by no more than 2 gradations of 8-bit color from the theoretically perfect curve.
const SAMPLE_THRESHOLD: f64 = 2. / 255.;

/// The effective midpoint domain shared by sampling and rendering: NaN reads as the linear default, and extremes are bounded to `0.01..=0.99` so curves stay finite and cheap to subdivide.
fn sanitized_midpoint(midpoint: f64) -> f64 {
	if midpoint.is_nan() { 0.5 } else { midpoint.clamp(0.01, 0.99) }
}

/// Apply the midpoint curve to a normalized parameter `t` (0 to 1) given a `midpoint` (0 to 1, where 0.5 is linear).
fn apply_midpoint(t: f64, midpoint: f64) -> f64 {
	let midpoint = sanitized_midpoint(midpoint);
	if (midpoint - 0.5).abs() < 1e-6 {
		return t;
	}

	if midpoint < 0.5 {
		let q = -1. / (1. - midpoint).log2();
		1. - (1. - t).powf(q)
	} else {
		let p = -1. / midpoint.log2();
		t.powf(p)
	}
}

/// Calls a color-space-generic function with the `color` crate space matching a [`GradientSpace`] variant.
macro_rules! with_space {
	($gradient_space:expr, $function:ident $(, $argument:expr)* $(,)?) => {
		match $gradient_space {
			GradientSpace::OkLab => $function::<color::Oklab>($($argument),*),
			GradientSpace::OkLCh => $function::<color::Oklch>($($argument),*),
			GradientSpace::Lab => $function::<color::Lab>($($argument),*),
			GradientSpace::LCh => $function::<color::Lch>($($argument),*),
			GradientSpace::Hsl => $function::<color::Hsl>($($argument),*),
			GradientSpace::Hsv => $function::<Hsv>($($argument),*),
			GradientSpace::RgbLinear => $function::<color::LinearSrgb>($($argument),*),
			GradientSpace::RgbGamma => $function::<color::Srgb>($($argument),*),
		}
	};
}

/// Interpolates between two adjacent stops' colors at `t` across their interval, in the gradient's chosen color space.
pub fn interpolate_stop_colors(color_a: Color, color_b: Color, t: f32, gradient_space: GradientSpace, gradient_hue_direction: GradientHueDirection) -> Color {
	match gradient_space {
		GradientSpace::OkLab => lerp_in_space::<color::Oklab>(color_a, color_b, t, gradient_hue_direction),
		GradientSpace::OkLCh => lerp_in_space::<color::Oklch>(color_a, color_b, t, gradient_hue_direction),
		GradientSpace::Lab => lerp_in_space::<color::Lab>(color_a, color_b, t, gradient_hue_direction),
		GradientSpace::LCh => lerp_in_space::<color::Lch>(color_a, color_b, t, gradient_hue_direction),
		GradientSpace::Hsl => lerp_in_space::<color::Hsl>(color_a, color_b, t, gradient_hue_direction),
		GradientSpace::Hsv => lerp_in_space::<Hsv>(color_a, color_b, t, gradient_hue_direction),
		GradientSpace::RgbLinear => color_a.lerp(&color_b, t),
		GradientSpace::RgbGamma => color_a.lerp_gamma_srgb(&color_b, t),
	}
}

/// Mix two colors in the color space `CS`, with alpha interpolating linearly like the sRGB spaces.
/// Polar spaces arc through hue per the direction, with an achromatic endpoint's powerless hue adopting the other's per CSS.
/// The mix can land slightly outside the sRGB gamut; it stays unclamped here and clips at render encoding.
fn lerp_in_space<CS: color::ColorSpace>(color_a: Color, color_b: Color, t: f32, gradient_hue_direction: GradientHueDirection) -> Color {
	let mut a = CS::from_linear_srgb([color_a.r(), color_a.g(), color_a.b()]);
	let mut b = CS::from_linear_srgb([color_b.r(), color_b.g(), color_b.b()]);

	if let Some(hue_index) = space_hue_index::<CS>() {
		let achromatic = achromatic_chroma_threshold::<CS>();
		if a[1] < achromatic && b[1] >= achromatic {
			a[hue_index] = b[hue_index];
		}
		if b[1] < achromatic && a[1] >= achromatic {
			b[hue_index] = a[hue_index];
		}

		// The fixup applies to hues that the conversions already place in the 0 to 360 range
		b[hue_index] = a[hue_index] + hue_delta((b[hue_index] - a[hue_index]) as f64, gradient_hue_direction) as f32;
	}

	let mixed = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];

	let [red, green, blue] = CS::to_linear_srgb(mixed);
	Color::from_rgbaf32_unchecked(red, green, blue, color_a.a() + (color_b.a() - color_a.a()) * t)
}

/// The classic HSV cylinder as `[hue in degrees, saturation 0-100, value 0-100]`, implemented locally
/// since CSS Color 4 (and therefore the `color` crate) offers only its HWB reparameterization, which
/// mixes along different paths through shades and tones.
#[derive(Clone, Copy, Debug)]
struct Hsv;

impl color::ColorSpace for Hsv {
	const LAYOUT: color::ColorSpaceLayout = color::ColorSpaceLayout::HueFirst;

	const WHITE_COMPONENTS: [f32; 3] = [0., 0., 100.];

	fn to_linear_srgb([hue, saturation, value]: [f32; 3]) -> [f32; 3] {
		let (saturation, value) = (saturation / 100., value / 100.);
		let channel = |n: f32| {
			let k = (n + hue / 60.).rem_euclid(6.);
			value - value * saturation * k.min(4. - k).clamp(0., 1.)
		};
		color::Srgb::to_linear_srgb([channel(5.), channel(3.), channel(1.)])
	}

	fn from_linear_srgb(src: [f32; 3]) -> [f32; 3] {
		let [red, green, blue] = color::Srgb::from_linear_srgb(src);
		let max = red.max(green).max(blue);
		let delta = max - red.min(green).min(blue);

		let hue = if delta <= 0. {
			0.
		} else if max == red {
			60. * ((green - blue) / delta).rem_euclid(6.)
		} else if max == green {
			60. * ((blue - red) / delta + 2.)
		} else {
			60. * ((red - green) / delta + 4.)
		};
		let saturation = if max <= 0. { 0. } else { delta / max };

		[hue, saturation * 100., max * 100.]
	}

	fn clip([hue, saturation, value]: [f32; 3]) -> [f32; 3] {
		[hue, saturation.clamp(0., 100.), value.clamp(0., 100.)]
	}
}

/// The largest difference between two colors across their gamma sRGB channels, the 8-bit-adjacent measure that rendered output quantizes to.
fn max_gamma_channel_deviation(a: Color, b: Color) -> f64 {
	let (a, b) = (a.to_gamma_srgb_channels(), b.to_gamma_srgb_channels());
	(0..4).fold(0_f64, |max, i| max.max((a[i] - b[i]).abs() as f64))
}

/// A color's channels in the color space `CS`, alongside its straight alpha.
fn space_channels<CS: color::ColorSpace>(color: Color) -> [f64; 4] {
	let [x, y, z] = CS::from_linear_srgb([color.r(), color.g(), color.b()]);
	[x as f64, y as f64, z as f64, color.a() as f64]
}

/// The inverse of [`space_channels`], leaving an out-of-gamut result unclamped so it clips at render encoding.
fn color_from_space_channels<CS: color::ColorSpace>(channels: [f64; 4]) -> Color {
	let [red, green, blue] = CS::to_linear_srgb([channels[0] as f32, channels[1] as f32, channels[2] as f32]);
	Color::from_rgbaf32_unchecked(red, green, blue, channels[3] as f32)
}

/// The channel carrying hue in a polar space, or `None` for a rectangular one.
fn space_hue_index<CS: color::ColorSpace>() -> Option<usize> {
	match CS::LAYOUT {
		color::ColorSpaceLayout::HueFirst => Some(0),
		color::ColorSpaceLayout::HueThird => Some(2),
		_ => None,
	}
}

/// The chroma (or saturation, channel 1 in both polar layouts) below which a color counts as achromatic and
/// its hue as powerless, scaled to the space's lightness range so conversion noise stays below the threshold.
fn achromatic_chroma_threshold<CS: color::ColorSpace>() -> f32 {
	1e-4 * CS::WHITE_COMPONENTS.iter().fold(0_f32, |max, &component| max.max(component))
}

/// The CSS Color 4 hue fixup: the signed arc between two hues taking the route the direction asks for.
fn hue_delta(delta: f64, gradient_hue_direction: GradientHueDirection) -> f64 {
	match gradient_hue_direction {
		GradientHueDirection::Shorter => {
			if delta > 180. {
				delta - 360.
			} else if delta < -180. {
				delta + 360.
			} else {
				delta
			}
		}
		GradientHueDirection::Longer => {
			if 0. < delta && delta < 180. {
				delta - 360.
			} else if -180. < delta && delta <= 0. {
				delta + 360.
			} else {
				delta
			}
		}
		GradientHueDirection::Increasing => {
			if delta < 0. {
				delta + 360.
			} else {
				delta
			}
		}
		GradientHueDirection::Decreasing => {
			if delta > 0. {
				delta - 360.
			} else {
				delta
			}
		}
	}
}

/// Every knot's color in the space `CS`. An achromatic knot borrows its nearest chromatic neighbor's powerless hue,
/// then the whole-hue run accumulates its per-step fixup so the spline sees one continuous sequence rather than values that wrap at 360.
fn knot_channels<CS: color::ColorSpace>(knots: &[GradientStop], gradient_hue_direction: GradientHueDirection) -> Vec<[f64; 4]> {
	let mut channels: Vec<[f64; 4]> = knots.iter().map(|knot| space_channels::<CS>(knot.color)).collect();

	let Some(hue_index) = space_hue_index::<CS>() else { return channels };

	let achromatic = achromatic_chroma_threshold::<CS>() as f64;
	let chromatic: Vec<usize> = (0..channels.len()).filter(|&index| channels[index][1] >= achromatic).collect();
	if chromatic.is_empty() {
		return channels;
	}

	for index in 0..channels.len() {
		if channels[index][1] < achromatic
			&& let Some(&nearest) = chromatic.iter().min_by_key(|&&other| other.abs_diff(index))
		{
			channels[index][hue_index] = channels[nearest][hue_index];
		}
	}

	for index in 1..channels.len() {
		let previous = channels[index - 1][hue_index];
		let delta = hue_delta(channels[index][hue_index].rem_euclid(360.) - previous.rem_euclid(360.), gradient_hue_direction);
		channels[index][hue_index] = previous + delta;
	}

	channels
}

/// A Piecewise Cubic Hermite Interpolating Polynomial (PCHIP) spline, preserving its samples' monotonicity:
/// it passes through every sample and joins the pieces with matching slopes, while the Fritsch-Carlson
/// limiter keeps each piece bounded by its own two samples, so the curve rises and falls only where its
/// samples do instead of overshooting past them.
///
/// Construction is O(n) and evaluation is O(log n) in the sample count.
struct MonotonicSpline {
	position: Vec<f64>,
	value: Vec<f64>,
	tangent: Vec<f64>,
}

impl MonotonicSpline {
	fn new(position: Vec<f64>, value: Vec<f64>) -> Self {
		let count = position.len();
		if count < 2 {
			let tangent = vec![0.; count];
			return Self { position, value, tangent };
		}

		let secant: Vec<f64> = (0..count - 1)
			.map(|index| {
				let run = position[index + 1] - position[index];
				if run.abs() < f64::EPSILON { 0. } else { (value[index + 1] - value[index]) / run }
			})
			.collect();

		// A sign change or a flat run between neighboring secants pins that tangent to zero,
		// which is what stops the curve from bulging past a local extreme
		let mut tangent = Vec::with_capacity(count);
		tangent.push(secant[0]);
		for index in 1..count - 1 {
			let (before, after) = (secant[index - 1], secant[index]);
			tangent.push(if before * after <= 0. { 0. } else { (before + after) / 2. });
		}
		tangent.push(secant[count - 2]);

		// Fritsch-Carlson: pull any tangent pair back inside the radius-3 circle around their shared secant
		for index in 0..count - 1 {
			if secant[index].abs() < f64::EPSILON {
				tangent[index] = 0.;
				tangent[index + 1] = 0.;
				continue;
			}

			let (alpha, beta) = (tangent[index] / secant[index], tangent[index + 1] / secant[index]);
			let magnitude = alpha * alpha + beta * beta;
			if magnitude > 9. {
				let scale = 3. / magnitude.sqrt();
				tangent[index] = scale * alpha * secant[index];
				tangent[index + 1] = scale * beta * secant[index];
			}
		}

		Self { position, value, tangent }
	}

	fn evaluate(&self, at: f64) -> f64 {
		let count = self.position.len();
		if count == 0 {
			return 0.;
		}
		if count == 1 || at <= self.position[0] {
			return self.value[0];
		}
		if at >= self.position[count - 1] {
			return self.value[count - 1];
		}

		let index = self.position.partition_point(|&position| position <= at).clamp(1, count - 1) - 1;
		let run = self.position[index + 1] - self.position[index];
		if run.abs() < f64::EPSILON {
			return self.value[index + 1];
		}

		let t = (at - self.position[index]) / run;
		let (t2, t3) = (t * t, t * t * t);

		(2. * t3 - 3. * t2 + 1.) * self.value[index] + (t3 - 2. * t2 + t) * run * self.tangent[index] + (-2. * t3 + 3. * t2) * self.value[index + 1] + (t3 - t2) * run * self.tangent[index + 1]
	}
}

/// The Smooth path: a monoticity-preserving spline per color channel through every stop, traversed by a second such spline that
/// maps ramp position to spline parameter. Fitting the stop and midpoint constraints into one global warp is what keeps the
/// traversal rate continuous across stops, where independent per-interval curves (what Linear uses) would kink at each one.
struct SmoothPath {
	space: GradientSpace,
	hue_index: Option<usize>,
	channel: [MonotonicSpline; 4],
	warp: MonotonicSpline,
}

impl SmoothPath {
	fn new(stops: &[GradientStop], settings: GradientSettings) -> Self {
		// A cyclic ramp gains two wrapped copies of each end, enough that the tangent estimate on either side of the
		// 1|0 boundary sees the same neighborhood and the seam joins smoothly. Stops at both 0 and 1 leave a zero-length
		// wrapped interval: that seam is a hard jump, and its copies would land on the real end stops, so it gets none.
		let count = stops.len();
		let wrapped_interval = count >= 2 && stops[0].position + 1. - stops[count - 1].position > f64::EPSILON;
		let mut knots: Vec<GradientStop> = Vec::with_capacity(count + 4);
		if settings.cyclic && wrapped_interval {
			knots.push(GradientStop {
				position: stops[count - 2].position - 1.,
				..stops[count - 2]
			});
			knots.push(GradientStop {
				position: stops[count - 1].position - 1.,
				..stops[count - 1]
			});
			knots.extend_from_slice(stops);
			knots.push(GradientStop {
				position: stops[0].position + 1.,
				..stops[0]
			});
			knots.push(GradientStop {
				position: stops[1].position + 1.,
				..stops[1]
			});
		} else {
			knots.extend_from_slice(stops);
		}

		let channels = with_space!(settings.space, knot_channels, &knots, settings.hue_direction);
		let parameter: Vec<f64> = (0..knots.len()).map(|index| index as f64).collect();
		let channel = std::array::from_fn(|component| MonotonicSpline::new(parameter.clone(), channels.iter().map(|values| values[component]).collect()));

		// Each stop pins its own knot parameter and each midpoint the half-parameter between two,
		// so one monotonic curve satisfies every midpoint constraint at once
		let mut warp_position = Vec::with_capacity(knots.len() * 2);
		let mut warp_value = Vec::with_capacity(knots.len() * 2);
		for (index, knot) in knots.iter().enumerate() {
			warp_position.push(knot.position);
			warp_value.push(index as f64);

			if let Some(next) = knots.get(index + 1) {
				let midpoint = knot.position + sanitized_midpoint(knot.midpoint) * (next.position - knot.position);
				if midpoint > knot.position && midpoint < next.position {
					warp_position.push(midpoint);
					warp_value.push(index as f64 + 0.5);
				}
			}
		}

		Self {
			space: settings.space,
			hue_index: with_space!(settings.space, space_hue_index),
			channel,
			warp: MonotonicSpline::new(warp_position, warp_value),
		}
	}

	fn evaluate(&self, t: f64) -> Color {
		let parameter = self.warp.evaluate(t);
		let mut channels: [f64; 4] = std::array::from_fn(|component| self.channel[component].evaluate(parameter));
		if let Some(hue_index) = self.hue_index {
			channels[hue_index] = channels[hue_index].rem_euclid(360.);
		}

		with_space!(self.space, color_from_space_channels, channels)
	}
}

/// Stepped holds each stop's color the whole way to the next stop, so the ramp jumps at stops and midpoints are ignored.
fn stepped_color(stops: &[GradientStop], t: f64, gradient_cyclic: bool) -> Color {
	let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return Color::BLACK };

	// Before the first stop a cyclic ramp is still inside the wrapped interval, which the last stop's color holds
	if t < first.position {
		return if gradient_cyclic { last.color } else { first.color };
	}
	if t >= last.position {
		return last.color;
	}

	stops.windows(2).find(|pair| t < pair[1].position).map_or(last.color, |pair| pair[0].color)
}

/// Linear traces the chord from each stop to the next, turning a corner at every stop, with the midpoint biasing the timing across each interval independently.
fn linear_color(stops: &[GradientStop], t: f64, settings: GradientSettings) -> Color {
	let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return Color::BLACK };

	if settings.cyclic && (t < first.position || t > last.position) {
		let wrap_length = first.position + 1. - last.position;
		if wrap_length <= f64::EPSILON {
			return first.color;
		}

		let local = if t >= last.position { t - last.position } else { t + 1. - last.position };
		let adjusted_t = apply_midpoint(local / wrap_length, last.midpoint);
		return interpolate_stop_colors(last.color, first.color, adjusted_t as f32, settings.space, settings.hue_direction);
	}

	if t <= first.position {
		return first.color;
	}
	if t >= last.position {
		return last.color;
	}

	for pair in stops.windows(2) {
		let (a, b) = (&pair[0], &pair[1]);
		if t >= a.position && t <= b.position {
			let normalized_t = (t - a.position) / (b.position - a.position);
			let adjusted_t = apply_midpoint(normalized_t, a.midpoint);
			return interpolate_stop_colors(a.color, b.color, adjusted_t as f32, settings.space, settings.hue_direction);
		}
	}

	Color::BLACK
}

/// Stepped's bake is exact rather than approximated: each interval emits its color at both ends,
/// leaving the renderer's own interpolation nothing to traverse so the jump lands squarely on the next stop.
fn stepped_samples(stops: &[GradientStop], gradient_cyclic: bool) -> Vec<(f64, Color, Option<f64>)> {
	let count = stops.len();
	let (first, last) = (&stops[0], &stops[count - 1]);
	let mut result = Vec::with_capacity(count * 2 + 2);

	// The wrapped interval holds the last stop's color from the 1|0 boundary through to the first stop
	if gradient_cyclic && first.position > 0. {
		result.push((0., last.color, None));
		result.push((first.position, last.color, None));
	}

	for pair in stops.windows(2) {
		result.push((pair[0].position, pair[0].color, Some(0.5)));
		result.push((pair[1].position, pair[0].color, None));
	}
	result.push((last.position, last.color, Some(0.5)));

	if gradient_cyclic && last.position < 1. {
		result.push((1., last.color, None));
	}

	result
}

/// Smooth's bake: anchor every stop, then subdivide between anchors until the renderer's gamma segments track the spline.
fn smooth_samples(stops: &[GradientStop], settings: GradientSettings) -> Vec<(f64, Color, Option<f64>)> {
	fn subdivide(path: &SmoothPath, left: f64, right: f64, color_left: Color, color_right: Color, result: &mut Vec<(f64, Color, Option<f64>)>, depth: u32) {
		const MAX_DEPTH: u32 = 20;
		if depth >= MAX_DEPTH {
			return;
		}

		// Probe the quarter points as well as the center, since a space with a steep toe peaks its deviation off-center
		let deviates = [0.25, 0.5, 0.75].into_iter().any(|fraction| {
			let probe = path.evaluate(left + (right - left) * fraction);
			max_gamma_channel_deviation(probe, color_left.lerp_gamma_srgb(&color_right, fraction as f32)) > SAMPLE_THRESHOLD
		});
		if !deviates {
			return;
		}

		let mid = (left + right) / 2.;
		let color_mid = path.evaluate(mid);
		subdivide(path, left, mid, color_left, color_mid, result, depth + 1);
		result.push((mid, color_mid, None));
		subdivide(path, mid, right, color_mid, color_right, result, depth + 1);
	}

	let path = SmoothPath::new(stops, settings);
	let count = stops.len();

	// A cyclic ramp's baked list runs boundary to boundary so downstream renderers stay unaware of the cycle
	let mut anchors: Vec<(f64, Option<f64>)> = Vec::with_capacity(count + 2);
	if settings.cyclic && stops[0].position > 0. {
		anchors.push((0., None));
	}
	anchors.extend(stops.iter().map(|stop| (stop.position, Some(sanitized_midpoint(stop.midpoint)))));
	let synthetic_end = settings.cyclic && stops[count - 1].position < 1.;
	if synthetic_end {
		anchors.push((1., None));
	}

	// A synthetic end anchor copies the start's color so the seam closes exactly rather than relying on the
	// two wrapped views of the spline agreeing to the last bit; a real stop at 1 keeps its own color
	let seam_color = synthetic_end.then(|| path.evaluate(anchors[0].0));
	let anchor_color = |position: f64| match seam_color {
		Some(color) if position >= 1. => color,
		_ => path.evaluate(position),
	};

	let mut result: Vec<(f64, Color, Option<f64>)> = Vec::new();
	for (index, &(position, midpoint)) in anchors.iter().enumerate() {
		let color = anchor_color(position);
		result.push((position, color, midpoint));

		if let Some(&(next, _)) = anchors.get(index + 1) {
			subdivide(&path, position, next, color, anchor_color(next), &mut result, 0);
		}
	}

	result
}

/// Clips a baked sample list to the unit range, replacing everything beyond an end with one sample interpolated at that boundary.
/// Renderers only accept offsets from 0 to 1, and the bake is dense enough that interpolating between neighbors in gamma reproduces the ramp's curve there.
fn clip_samples_to_unit_range(samples: &[(f64, Color, Option<f64>)]) -> Vec<(f64, Color, Option<f64>)> {
	fn sample_at(samples: &[(f64, Color, Option<f64>)], boundary: f64) -> Color {
		let index = samples.partition_point(|&(position, ..)| position < boundary);

		// A sample sitting on the boundary already answers it, so only a straddling pair needs interpolating
		if let Some(&(position, color, _)) = samples.get(index)
			&& position == boundary
		{
			return color;
		}

		match (index.checked_sub(1).map(|before| samples[before]), samples.get(index).copied()) {
			(Some((left, left_color, _)), Some((right, right_color, _))) => {
				let span = right - left;
				let fraction = if span > 0. { (boundary - left) / span } else { 0. };
				left_color.lerp_gamma_srgb(&right_color, fraction as f32)
			}
			(Some((_, color, _)), None) | (None, Some((_, color, _))) => color,
			(None, None) => Color::BLACK,
		}
	}

	let leads_in = samples.first().is_some_and(|&(position, ..)| position < 0.);
	let trails_out = samples.last().is_some_and(|&(position, ..)| position > 1.);
	if !leads_in && !trails_out {
		return samples.to_vec();
	}

	let mut result: Vec<(f64, Color, Option<f64>)> = samples.iter().copied().filter(|&(position, ..)| (0. ..=1.).contains(&position)).collect();
	if leads_in && result.first().is_none_or(|&(position, ..)| position > 0.) {
		result.insert(0, (0., sample_at(samples, 0.), None));
	}
	if trails_out && result.last().is_none_or(|&(position, ..)| position < 1.) {
		result.push((1., sample_at(samples, 1.), None));
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
	pub position: f64,
	pub midpoint: f64,
	pub color: Color,
}

pub struct GradientStopsIter<'a> {
	stops: &'a Gradient,
	index: usize,
}

impl Iterator for GradientStopsIter<'_> {
	type Item = GradientStop;

	fn next(&mut self) -> Option<Self::Item> {
		// Elided positions fall back to the non-cyclic even distribution; cyclic-aware callers should read `position(index, gradient_cyclic)` instead
		let stop = GradientStop {
			position: self.stops.position(self.index, false),
			midpoint: self.stops.midpoint(self.index),
			color: self.stops.color(self.index)?,
		};
		self.index += 1;
		Some(stop)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.stops.len().saturating_sub(self.index);
		(remaining, Some(remaining))
	}
}

impl ExactSizeIterator for GradientStopsIter<'_> {}

impl<'a> IntoIterator for &'a Gradient {
	type Item = GradientStop;
	type IntoIter = GradientStopsIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		GradientStopsIter { stops: self, index: 0 }
	}
}

impl IntoIterator for Gradient {
	type Item = GradientStop;
	type IntoIter = std::vec::IntoIter<GradientStop>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter().collect::<Vec<_>>().into_iter()
	}
}

/// The fallback position of the gradient stop at `index` when no `position` attribute exists, where all `count` stops are spaced evenly from 0 to 1.
fn even_position(index: usize, count: usize, gradient_cyclic: bool) -> f64 {
	if count <= 1 {
		return 0.;
	}
	// A cyclic gradient reserves the same span for the wrapped interval as for each interval between stops
	let denominator = if gradient_cyclic { count } else { count - 1 };
	index as f64 / denominator as f64
}

impl Gradient {
	pub fn new(stops: impl IntoIterator<Item = GradientStop>) -> Self {
		let stops: Vec<GradientStop> = stops.into_iter().collect();
		let mut list: List<Color> = stops.iter().map(|stop| Item::new_from_element(stop.color)).collect();

		for (index, stop) in stops.iter().enumerate() {
			list.set_attribute(ATTR_POSITION, index, stop.position);
			list.set_attribute(ATTR_MIDPOINT, index, stop.midpoint);
		}

		Self(list)
	}

	pub fn black_to_white() -> Self {
		Self::from(vec![Color::BLACK, Color::WHITE])
	}

	pub fn as_color_list(&self) -> &List<Color> {
		&self.0
	}

	pub fn into_color_list(self) -> List<Color> {
		self.0
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn iter(&self) -> GradientStopsIter<'_> {
		self.into_iter()
	}

	/// The color of the stop at the given index, if in bounds.
	pub fn color(&self, index: usize) -> Option<Color> {
		self.0.element(index).copied()
	}

	/// The effective position of the stop at the given index: its `position` attribute value, or its share of an even distribution when the attribute is absent.
	pub fn position(&self, index: usize, gradient_cyclic: bool) -> f64 {
		self.0
			.attribute::<f64>(ATTR_POSITION, index)
			.copied()
			.unwrap_or_else(|| even_position(index, self.len(), gradient_cyclic))
	}

	/// The effective midpoint of the stop at the given index: its `midpoint` attribute value, or the linear interpolation default of `0.5` when the attribute is absent.
	pub fn midpoint(&self, index: usize) -> f64 {
		self.0.attribute::<f64>(ATTR_MIDPOINT, index).copied().unwrap_or(0.5)
	}

	/// The effective positions of all stops.
	pub fn positions(&self, gradient_cyclic: bool) -> Vec<f64> {
		(0..self.len()).map(|index| self.position(index, gradient_cyclic)).collect()
	}

	/// The effective midpoints of all stops.
	pub fn midpoints(&self) -> Vec<f64> {
		(0..self.len()).map(|index| self.midpoint(index)).collect()
	}

	/// Whether the `position` attribute is explicitly present rather than falling back to the even distribution.
	pub fn has_position_attribute(&self) -> bool {
		self.0.iter_attribute_values::<f64>(ATTR_POSITION).is_some()
	}

	/// Whether the `midpoint` attribute is explicitly present rather than falling back to the linear interpolation default.
	pub fn has_midpoint_attribute(&self) -> bool {
		self.0.iter_attribute_values::<f64>(ATTR_MIDPOINT).is_some()
	}

	/// The `position` attribute's values when present, or `None` when the stops fall back to the even distribution.
	fn position_attribute(&self) -> Option<Vec<f64>> {
		self.0.iter_attribute_values::<f64>(ATTR_POSITION).map(|values| values.copied().collect())
	}

	/// The `midpoint` attribute's values when present, or `None` when the stops fall back to the linear interpolation default.
	fn midpoint_attribute(&self) -> Option<Vec<f64>> {
		self.0.iter_attribute_values::<f64>(ATTR_MIDPOINT).map(|values| values.copied().collect())
	}

	/// The `position` attribute when present and meaningfully different from the even distribution, which is the form worth persisting in the graph.
	pub fn nondefault_positions(&self, gradient_cyclic: bool) -> Option<Vec<f64>> {
		let positions = self.position_attribute()?;
		let count = self.len();
		positions
			.iter()
			.enumerate()
			.any(|(index, &position)| !position.is_finite() || (position - even_position(index, count, gradient_cyclic)).abs() > 1e-6)
			.then_some(positions)
	}

	/// The `midpoint` attribute when present and meaningfully different from the linear interpolation default of `0.5`.
	pub fn nondefault_midpoints(&self) -> Option<Vec<f64>> {
		let midpoints = self.midpoint_attribute()?;
		midpoints.iter().any(|&midpoint| (midpoint - 0.5).abs() > 1e-6).then_some(midpoints)
	}

	/// Removes the `position`/`midpoint` attributes when they merely restate the defaults, restoring the canonical absence-as-default form.
	pub fn elide_default_attributes(&mut self, gradient_cyclic: bool) {
		if self.has_position_attribute() && self.nondefault_positions(gradient_cyclic).is_none() {
			self.0.remove_attribute(ATTR_POSITION);
		}
		if self.has_midpoint_attribute() && self.nondefault_midpoints().is_none() {
			self.0.remove_attribute(ATTR_MIDPOINT);
		}
	}

	/// Pins the stops to the positions they currently occupy under `from_cyclic`, then re-elides against `to_cyclic`,
	/// so flipping the flag leaves them where they are instead of snapping to the other mode's even distribution.
	pub fn hold_positions_across_cyclic_change(&mut self, from_cyclic: bool, to_cyclic: bool) {
		if from_cyclic == to_cyclic {
			return;
		}

		self.materialize_default_positions(from_cyclic);
		self.elide_default_attributes(to_cyclic);
	}

	/// Writes the whole `position` attribute from the effective values, since the even-distribution default is index-dependent and can't be produced by cell-wise padding.
	fn materialize_default_positions(&mut self, gradient_cyclic: bool) {
		if self.has_position_attribute() {
			return;
		}

		let count = self.len();
		for index in 0..count {
			self.0.set_attribute(ATTR_POSITION, index, even_position(index, count, gradient_cyclic));
		}
	}

	/// Replaces the color of the stop at `index`, if it exists.
	pub fn set_color(&mut self, index: usize, color: Color) {
		if let Some(element) = self.0.element_mut(index) {
			*element = color;
		}
	}

	/// Sets the position of the stop at `index`, if it exists, materializing the whole `position` attribute so the other stops keep their effective placements.
	pub fn set_position(&mut self, index: usize, position: f64, gradient_cyclic: bool) {
		if index >= self.len() {
			return;
		}
		self.materialize_default_positions(gradient_cyclic);
		self.0.set_attribute(ATTR_POSITION, index, position);
	}

	/// Sets the midpoint of the stop at `index`, if it exists.
	pub fn set_midpoint(&mut self, index: usize, midpoint: f64) {
		if index >= self.len() {
			return;
		}
		self.0.set_attribute(ATTR_MIDPOINT, index, midpoint);
	}

	/// Replaces the `position` attribute with the given values, padding with the final value if fewer than the stop count and ignoring any extras.
	/// An empty list removes the attribute, restoring even distribution.
	pub fn set_positions(&mut self, positions: &[f64]) {
		let Some(&last) = positions.last() else {
			self.0.remove_attribute(ATTR_POSITION);
			return;
		};

		for index in 0..self.len() {
			self.0.set_attribute(ATTR_POSITION, index, positions.get(index).copied().unwrap_or(last));
		}
	}

	/// Replaces the `midpoint` attribute with the given values, padding with the final value if fewer than the stop count and ignoring any extras.
	/// An empty list removes the attribute, restoring the linear interpolation default of `0.5` for every stop.
	pub fn set_midpoints(&mut self, midpoints: &[f64]) {
		let Some(&last) = midpoints.last() else {
			self.0.remove_attribute(ATTR_MIDPOINT);
			return;
		};

		for index in 0..self.len() {
			self.0.set_attribute(ATTR_MIDPOINT, index, midpoints.get(index).copied().unwrap_or(last));
		}
	}

	/// Rebuilds the stop list from the given stop indices, preserving every attribute.
	fn reordered(&self, indices: impl IntoIterator<Item = usize>) -> List<Color> {
		let mut list = List::new();
		for index in indices {
			if let Some(item) = self.0.clone_item(index) {
				list.push(item);
			}
		}
		list
	}

	/// Remove a stop at the given index.
	pub fn remove(&mut self, index: usize) {
		self.0 = self.reordered((0..self.len()).filter(|&i| i != index));
	}

	/// Remove and return the last stop's color, or `None` if empty.
	pub fn pop(&mut self) -> Option<Color> {
		let color = self.color(self.len().checked_sub(1)?);
		self.0 = self.reordered(0..self.len() - 1);
		color
	}

	/// Move the stop at `index` to a new position, re-sorting the stops by position. Returns the new index of the moved stop.
	pub fn move_stop(&mut self, index: usize, position: f64, gradient_cyclic: bool) -> usize {
		if index >= self.len() {
			return index;
		}
		self.set_position(index, position, gradient_cyclic);
		self.sort_returning_new_index(index, gradient_cyclic)
	}

	/// Insert a new stop at the given position, sampling the gradient at that position to determine the new stop's color.
	/// The new stop's midpoint is inherited from the interval it splits (or `0.5` if inserting at the very start of a non-cyclic gradient).
	/// Returns the index where the new stop was inserted.
	pub fn insert_stop(&mut self, position: f64, settings: GradientSettings) -> usize {
		let gradient_cyclic = settings.cyclic;
		// The sampled position is inside the ramp, where the spread must act as Pad (Repeat would wrap an exact 1 onto the first stop)
		let color = self.evaluate(
			position,
			GradientSettings {
				spread: Default::default(),
				..settings
			},
		);
		let index = (0..self.len()).position(|i| self.position(i, gradient_cyclic) > position).unwrap_or(self.len());

		// Inserting before the first stop of a cyclic gradient splits the wrapped interval, so its handle is inherited
		let midpoint = if index > 0 {
			self.midpoint(index - 1)
		} else if gradient_cyclic && !self.is_empty() {
			self.midpoint(self.len() - 1)
		} else {
			0.5
		};

		self.insert_stop_values(position, midpoint, color, gradient_cyclic)
	}

	/// Insert a copy of the stop at `source_index` (same color and midpoint) at `position`, keeping the stops sorted by position.
	/// Returns the index where the copy was inserted, or `None` if `source_index` is out of range.
	pub fn duplicate_stop(&mut self, source_index: usize, position: f64, gradient_cyclic: bool) -> Option<usize> {
		let color = self.color(source_index)?;
		let midpoint = self.midpoint(source_index);
		Some(self.insert_stop_values(position, midpoint, color, gradient_cyclic))
	}

	/// Splices a new stop into the sorted position, materializing explicit positions (an arbitrary insertion breaks even distribution)
	/// while giving the new stop a midpoint cell only if the attribute already exists.
	fn insert_stop_values(&mut self, position: f64, midpoint: f64, color: Color, gradient_cyclic: bool) -> usize {
		self.materialize_default_positions(gradient_cyclic);
		let index = (0..self.len()).position(|i| self.position(i, gradient_cyclic) > position).unwrap_or(self.len());

		let mut item = Item::new_from_element(color).with_attribute(ATTR_POSITION, position);
		if self.has_midpoint_attribute() {
			item = item.with_attribute(ATTR_MIDPOINT, midpoint);
		}

		let mut list = self.reordered(0..index);
		list.push(item);
		for i in index..self.len() {
			if let Some(existing) = self.0.clone_item(i) {
				list.push(existing);
			}
		}

		self.0 = list;
		index
	}

	/// Reset the midpoint for the interval starting at `index` to its default `0.5`.
	pub fn reset_midpoint(&mut self, index: usize) {
		if self.has_midpoint_attribute() && index < self.len() {
			self.0.set_attribute(ATTR_MIDPOINT, index, 0.5);
		}
	}

	/// Sort the stops in place by position; returns the new index of the stop that was at `previous_index` before sorting.
	fn sort_returning_new_index(&mut self, previous_index: usize, gradient_cyclic: bool) -> usize {
		// An absent position attribute is an even distribution, which is already sorted
		if !self.has_position_attribute() {
			return previous_index;
		}

		let mut indices: Vec<usize> = (0..self.len()).collect();
		indices.sort_by(|&a, &b| self.position(a, gradient_cyclic).total_cmp(&self.position(b, gradient_cyclic)));
		let new_index = indices.iter().position(|&i| i == previous_index).unwrap_or(previous_index);
		self.0 = self.reordered(indices);
		new_index
	}

	/// Gradient stops as evaluation and rendering should see them, sorted ascending: a NaN drops its stop since it has no
	/// defined placement, and an infinity lands on the nearest end. A cyclic ramp folds positions into the 0 to 1 range, the
	/// whole of its circle, while a non-cyclic ramp keeps finite ones so a shifted ramp samples through stops that slid out.
	fn normalized_stops(&self, gradient_cyclic: bool) -> Vec<GradientStop> {
		let mut stops: Vec<GradientStop> = (0..self.len())
			.filter_map(|index| {
				let position = self.position(index, gradient_cyclic);
				if position.is_nan() {
					return None;
				}
				let position = if gradient_cyclic || position.is_infinite() { position.clamp(0., 1.) } else { position };

				let midpoint = self.midpoint(index);
				let color = self.color(index)?;

				Some(GradientStop { position, midpoint, color })
			})
			.collect();

		stops.sort_by(|a, b| a.position.total_cmp(&b.position));
		stops
	}

	/// Samples the gradient's color at `t`. Given a `t` outside the 0 to 1 range, the spread determines how the gradient extends.
	///
	/// Each call rebuilds the sampling state, so loops over many `t` values should hold a [`Gradient::evaluator`] instead.
	pub fn evaluate(&self, t: f64, settings: GradientSettings) -> Color {
		self.evaluator(settings).evaluate(t)
	}

	/// Prepares the gradient for repeated sampling: the stop normalization and any Smooth spline construction, together
	/// O(n log n) in the stop count, happen once here rather than on every [`GradientEvaluator::evaluate`] call.
	pub fn evaluator(&self, settings: GradientSettings) -> GradientEvaluator {
		let stops = self.normalized_stops(settings.cyclic);
		// A spline through fewer than two stops has nothing to curve between, leaving those ramps to linear evaluation
		let smooth_path = (settings.interpolation == GradientInterpolation::Smooth && stops.len() >= 2).then(|| SmoothPath::new(&stops, settings));
		GradientEvaluator { stops, settings, smooth_path }
	}

	pub fn sort(&mut self, gradient_cyclic: bool) {
		self.sort_returning_new_index(0, gradient_cyclic);
	}

	pub fn reversed(&self, gradient_cyclic: bool) -> Self {
		let count = self.len();
		let mut list = self.mirrored_rows(gradient_cyclic);

		// Row reversal already reversed the position cells' order, each also flips across the range
		if self.has_position_attribute()
			&& let Some(positions) = list.iter_attribute_values_mut::<f64>(ATTR_POSITION)
		{
			for position in positions {
				*position = 1. - *position;
			}
		} else if gradient_cyclic && !self.has_position_attribute() {
			// The cyclic even distribution starts at 0 rather than being symmetric across the range, so its flip must materialize
			for index in 0..count {
				list.set_attribute(ATTR_POSITION, index, 1. - even_position(count - 1 - index, count, true));
			}
		}

		Self(list)
	}

	/// Reverses the rows for a mirrored ramp, leaving positions to the caller since each mirroring has its own axis. Midpoints
	/// govern the interval to their stop's right, so each shifts by one stop as well as flipping. The wrap handle flips in place.
	fn mirrored_rows(&self, gradient_cyclic: bool) -> List<Color> {
		let count = self.len();
		let mut list = self.reordered((0..count).rev());

		if self.has_midpoint_attribute() {
			let midpoints: Vec<f64> = (0..count)
				.map(|i| {
					if i + 1 < count {
						1. - self.midpoint(count - 2 - i)
					} else if gradient_cyclic {
						1. - self.midpoint(count - 1)
					} else {
						0.5
					}
				})
				.collect();
			for (index, midpoint) in midpoints.into_iter().enumerate() {
				list.set_attribute(ATTR_MIDPOINT, index, midpoint);
			}
		}

		list
	}

	/// Shifts every stop along the ramp by `fraction` of its whole length, each midpoint riding along. A cyclic ramp rotates,
	/// wrapping past 1 back to 0 and re-sorting. A non-cyclic one translates, its positions leaving the range to sample through.
	pub fn shift_positions(&mut self, fraction: f64, gradient_cyclic: bool) {
		if !fraction.is_finite() {
			return;
		}

		// Only a rotation comes back around every whole turn, while a translation keeps going
		let shift = if gradient_cyclic { fraction.rem_euclid(1.) } else { fraction };
		if shift == 0. {
			return;
		}

		self.materialize_default_positions(gradient_cyclic);

		if let Some(positions) = self.0.iter_attribute_values_mut::<f64>(ATTR_POSITION) {
			for position in positions {
				// A non-finite position has no placement to shift, and wrapping one would land it on NaN
				if !position.is_finite() {
					continue;
				}

				*position = if gradient_cyclic {
					// Wrapping into the range first lands stops at 0 and 1, the same circle point,
					// on an identical value rather than rounding apart into an unstable order
					(position.rem_euclid(1.) + shift).rem_euclid(1.)
				} else {
					*position + shift
				};
			}
		}

		// Rotation carries stops across the seam and reorders the list, but translation leaves the order alone
		if gradient_cyclic {
			self.sort(gradient_cyclic);
		}

		self.elide_default_attributes(gradient_cyclic);
	}

	/// Multiplies every stop's distance from `pivot` by `factor`, a negative one mirroring the ramp across it. A cyclic ramp wraps
	/// the results back around the circle its positions live on. A non-cyclic ramp lets them leave the range and samples through.
	pub fn stretch_positions(&mut self, factor: f64, pivot: f64, gradient_cyclic: bool) {
		if !factor.is_finite() || !pivot.is_finite() || factor == 1. {
			return;
		}

		self.materialize_default_positions(gradient_cyclic);

		if let Some(positions) = self.0.iter_attribute_values_mut::<f64>(ATTR_POSITION) {
			for position in positions {
				if !position.is_finite() {
					continue;
				}

				let stretched = pivot + (*position - pivot) * factor;
				*position = if gradient_cyclic { stretched.rem_euclid(1.) } else { stretched };
			}
		}

		// A mirrored ramp comes out in descending order, which the row reversal undoes
		if factor < 0. {
			self.0 = self.mirrored_rows(gradient_cyclic);
		}

		// Wrapping can carry stops across the seam and reorder them
		if gradient_cyclic {
			self.sort(gradient_cyclic);
		}

		self.elide_default_attributes(gradient_cyclic);
	}

	pub fn map_colors<F: Fn(&Color) -> Color>(&self, f: F) -> Self {
		let mut mapped = self.clone();
		mapped.0.iter_element_values_mut().for_each(|color| *color = f(color));
		mapped
	}

	/// The gradient's [`Gradient::interpolated_samples`], falling back to one black sample when it has no stops, since a
	/// stopless SVG gradient paints nothing where [`Gradient::evaluate`] gives black.
	pub fn interpolated_samples_or_black(&self, settings: GradientSettings) -> Vec<(f64, Color, Option<f64>)> {
		let samples = self.interpolated_samples(settings);
		if samples.is_empty() { vec![(0., Color::BLACK, None)] } else { samples }
	}

	/// Build a CSS `background-image` value embedding the gradient as an SVG data URI, sampling the midpoint curves, color
	/// space, and spline. SVG interpolates its stops with straight alpha, matching the canvas renderers, where a CSS
	/// `linear-gradient` interpolates premultiplied and would hide the pull a transparent stop's RGB exerts on the render.
	pub fn to_svg_background_image(&self, settings: GradientSettings) -> String {
		use std::fmt::Write;

		let mut stops = String::new();
		for (position, color, _) in self.interpolated_samples_or_black(settings) {
			let srgba = SRGBA8::from(color);
			let _ = write!(stops, "<stop offset='{}' stop-color='#{}'", (position * 1e4).round() / 1e4, srgba.to_rgb_hex());
			if srgba.alpha < 255 {
				let _ = write!(stops, " stop-opacity='{}'", (color.a() as f64 * 1000.).round() / 1000.);
			}
			stops.push_str("/>");
		}

		// A sizeless SVG stretches to fill the CSS background area; the encoding covers the URI-hostile characters
		let svg = format!("<svg xmlns='http://www.w3.org/2000/svg'><linearGradient id='g' x1='0' y1='0' x2='1' y2='0'>{stops}</linearGradient><rect width='100%' height='100%' fill='url(#g)'/></svg>");
		let encoded = svg.replace('%', "%25").replace('#', "%23").replace('<', "%3C").replace('>', "%3E");
		format!("url(\"data:image/svg+xml,{encoded}\")")
	}

	/// Produce a set of linearly-interpolated color samples that approximate the gradient's true curve.
	///
	/// Each sample is `(position, color, original_midpoint)` where `original_midpoint` is `Some(f64)` with the corresponding
	/// midpoint for actual gradient stops, and `None` for synthesized curve approximation samples.
	///
	/// The downstream SVG/CSS and Vello renderers interpolate between adjacent emitted stops in gamma sRGB space, so the
	/// subdivision emits enough samples that the gamma-drawn segments match the ramp's true curve: the midpoint bias, the
	/// color space when it is not gamma itself, and the spline path when the ramp interpolates smoothly. Stepped is exact
	/// without any subdivision, since a pair of samples per interval reproduces its jumps.
	pub fn interpolated_samples(&self, settings: GradientSettings) -> Vec<(f64, Color, Option<f64>)> {
		#[allow(clippy::too_many_arguments)]
		fn subdivide(
			left: f64,
			right: f64,
			midpoint: f64,
			pos_a: f64,
			pos_b: f64,
			color_a: Color,
			color_b: Color,
			gradient_space: GradientSpace,
			gradient_hue_direction: GradientHueDirection,
			result: &mut Vec<(f64, Color, Option<f64>)>,
			depth: u32,
		) {
			const MAX_DEPTH: u32 = 20;
			if depth >= MAX_DEPTH {
				return;
			}

			let mid = (left + right) / 2.;

			let y_actual = apply_midpoint(mid, midpoint);
			let y_left = apply_midpoint(left, midpoint);
			let y_right = apply_midpoint(right, midpoint);
			let y_linear = (y_left + y_right) / 2.;

			// A sample is needed wherever the renderer's gamma segment between the flanking samples would stray
			// from the ramp's true curve: from the midpoint bias, or from a non-gamma space's own curvature.
			// The space check probes the quarter points as well as the center, since spaces with a steep toe
			// (like CIE Lab near black) peak their deviation off-center
			let midpoint_deviates = (y_actual - y_linear).abs() > SAMPLE_THRESHOLD;
			let space_deviates = gradient_space != GradientSpace::RgbGamma && {
				let color_left = interpolate_stop_colors(color_a, color_b, y_left as f32, gradient_space, gradient_hue_direction);
				let color_right = interpolate_stop_colors(color_a, color_b, y_right as f32, gradient_space, gradient_hue_direction);
				[0.25, 0.5, 0.75].into_iter().any(|fraction| {
					let y_probe = apply_midpoint(left + (right - left) * fraction, midpoint);
					let color_target = interpolate_stop_colors(color_a, color_b, y_probe as f32, gradient_space, gradient_hue_direction);
					max_gamma_channel_deviation(color_target, color_left.lerp_gamma_srgb(&color_right, fraction as f32)) > SAMPLE_THRESHOLD
				})
			};

			if midpoint_deviates || space_deviates {
				subdivide(left, mid, midpoint, pos_a, pos_b, color_a, color_b, gradient_space, gradient_hue_direction, result, depth + 1);

				let global_pos = pos_a + mid * (pos_b - pos_a);
				let color = interpolate_stop_colors(color_a, color_b, y_actual as f32, gradient_space, gradient_hue_direction);
				result.push((global_pos, color, None));

				subdivide(mid, right, midpoint, pos_a, pos_b, color_a, color_b, gradient_space, gradient_hue_direction, result, depth + 1);
			}
		}

		fn linear_samples(stops: &[GradientStop], settings: GradientSettings) -> Vec<(f64, Color, Option<f64>)> {
			let (gradient_space, gradient_hue_direction) = (settings.space, settings.hue_direction);
			let count = stops.len();
			let mut result = Vec::new();

			for i in 0..count - 1 {
				let pos_a = stops[i].position;
				let pos_b = stops[i + 1].position;
				let color_a = stops[i].color;
				let color_b = stops[i + 1].color;
				let midpoint = sanitized_midpoint(stops[i].midpoint);
				let next_midpoint = sanitized_midpoint(stops[i + 1].midpoint);

				// Add the start stop (subsequent intervals share the previous end stop)
				if i == 0 {
					result.push((pos_a, color_a, Some(midpoint)));
				}

				// Only subdivide if the midpoint deviates from linear (0.5) or a non-gamma space may curve away from the drawn gamma segment
				if (midpoint - 0.5).abs() >= 1e-6 || gradient_space != GradientSpace::RgbGamma {
					subdivide(0., 1., midpoint, pos_a, pos_b, color_a, color_b, gradient_space, gradient_hue_direction, &mut result, 0);
				}

				// Add the end stop
				result.push((pos_b, color_b, Some(next_midpoint)));
			}

			// Bake the wrapped interval into the flat list: the piece from the last stop to the 1|0 boundary, and the piece
			// continuing from the boundary to the first stop, so both ends of the emitted list share the boundary-crossing
			// color and downstream renderers stay unaware of the cycle
			if settings.cyclic {
				let (first, last) = (&stops[0], &stops[count - 1]);
				let wrap_length = first.position + 1. - last.position;
				if wrap_length > f64::EPSILON {
					let wrap_midpoint = sanitized_midpoint(last.midpoint);
					let boundary_fraction = (1. - last.position) / wrap_length;
					let y_boundary = apply_midpoint(boundary_fraction, wrap_midpoint);
					let boundary_color = interpolate_stop_colors(last.color, first.color, y_boundary as f32, gradient_space, gradient_hue_direction);

					if last.position < 1. {
						let virtual_end = last.position + wrap_length;
						subdivide(
							0.,
							boundary_fraction,
							wrap_midpoint,
							last.position,
							virtual_end,
							last.color,
							first.color,
							gradient_space,
							gradient_hue_direction,
							&mut result,
							0,
						);
						result.push((1., boundary_color, None));
					}

					if first.position > 0. {
						let virtual_start = last.position - 1.;
						let mut leading = vec![(0., boundary_color, None)];
						subdivide(
							boundary_fraction,
							1.,
							wrap_midpoint,
							virtual_start,
							first.position,
							last.color,
							first.color,
							gradient_space,
							gradient_hue_direction,
							&mut leading,
							0,
						);
						leading.append(&mut result);
						result = leading;
					}
				}
			}

			result
		}

		let stops = self.normalized_stops(settings.cyclic);
		let count = stops.len();
		if count == 0 {
			return vec![];
		}

		if count == 1 {
			let sample = vec![(stops[0].position, stops[0].color, Some(sanitized_midpoint(stops[0].midpoint)))];
			return clip_samples_to_unit_range(&sample);
		}

		let samples = match settings.interpolation {
			GradientInterpolation::Stepped => stepped_samples(&stops, settings.cyclic),
			GradientInterpolation::Linear => linear_samples(&stops, settings),
			GradientInterpolation::Smooth => smooth_samples(&stops, settings),
		};

		// A shifted non-cyclic ramp can leave stops outside the range, which the curve still runs through
		let mut result = clip_samples_to_unit_range(&samples);

		// If every midpoint is 0.5 (or within epsilon), turn all midpoints to None
		if result.iter().all(|(_, _, midpoint)| matches!(midpoint, Some(m) if (m - 0.5).abs() < 1e-6)) {
			result.iter_mut().for_each(|(_, _, midpoint)| *midpoint = None);
		}

		result
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let stops = self.iter().zip(other.iter()).map(|(a, b)| {
			let position = a.position + (b.position - a.position) * time;
			let color = a.color.lerp(&b.color, time as f32);
			GradientStop { position, midpoint: 0.5, color }
		});
		Gradient::new(stops)
	}
}

/// A gradient prepared for repeated sampling by [`Gradient::evaluator`], holding the normalized stops and any
/// prebuilt Smooth spline so each sample pays only the per-evaluation cost.
pub struct GradientEvaluator {
	stops: Vec<GradientStop>,
	settings: GradientSettings,
	smooth_path: Option<SmoothPath>,
}

impl GradientEvaluator {
	/// Samples the gradient's color at `t`. Given a `t` outside the 0 to 1 range, the spread determines how the gradient extends.
	pub fn evaluate(&self, t: f64) -> Color {
		let t = match self.settings.spread {
			GradientSpread::Pad => t.clamp(0., 1.),
			GradientSpread::Repeat => t.rem_euclid(1.),
			GradientSpread::Reflect => {
				let cycle = t.rem_euclid(2.);
				if cycle > 1. { 2. - cycle } else { cycle }
			}
			GradientSpread::Clear => {
				if !(0. ..=1.).contains(&t) {
					return Color::TRANSPARENT;
				}
				t
			}
		};

		match &self.smooth_path {
			Some(path) => path.evaluate(t),
			None if self.settings.interpolation == GradientInterpolation::Stepped => stepped_color(&self.stops, t, self.settings.cyclic),
			None => linear_color(&self.stops, t, self.settings),
		}
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum GradientSpread {
	/// Extends the end colors outward.
	#[default]
	#[icon("GradientSpreadPad")]
	Pad,
	/// Loops the gradient by mirroring back-and-forth.
	#[icon("GradientSpreadReflect")]
	Reflect,
	/// Loops the gradient as copies of itself.
	#[icon("GradientSpreadRepeat")]
	Repeat,
	/// Cuts off to transparency beyond the ends.
	#[icon("GradientSpreadClear")]
	Clear,
}

impl GradientSpread {
	pub fn svg_name(&self) -> &'static str {
		match self {
			GradientSpread::Pad => "pad",
			GradientSpread::Reflect => "reflect",
			GradientSpread::Repeat => "repeat",
			// SVG has no clear mode; renderers emulate it over pad with transparent guard stops
			GradientSpread::Clear => "pad",
		}
	}

	pub fn is_default(&self) -> bool {
		*self == Self::default()
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum GradientSpace {
	/// Interpolates between stops in the OkLab color space, the modern perceptual standard.
	#[default]
	#[label("Perceptual (OkLab)")]
	OkLab,
	/// Interpolates between stops in the CIE Lab color space, the traditional perceptual standard.
	#[label("Perceptual (Lab)")]
	Lab,
	/// Interpolates between stops in the polar form of OkLab, arcing through hue instead of fading through gray.
	#[label("Perceptual Hue (OkLCh)")]
	OkLCh,
	/// Interpolates between stops in the polar form of CIE Lab, arcing through hue instead of fading through gray.
	#[label("Perceptual Hue (LCh)")]
	LCh,
	/// Interpolates between stops in linear light, keeping transitions uniformly bright.
	#[menu_separator]
	#[label("Linear (RGB)")]
	RgbLinear,
	/// Interpolates between stops in gamma-encoded RGB, matching classic SVG and CSS gradients.
	#[label("Classic (RGB)")]
	RgbGamma,
	/// Interpolates between stops in the hue/saturation/value cylinder, keeping tints at full brightness.
	#[label("Classic Hue (HSV)")]
	Hsv,
	/// Interpolates between stops in the hue/saturation/lightness cylinder.
	#[label("Classic Hue (HSL)")]
	Hsl,
}

impl GradientSpace {
	pub fn is_default(&self) -> bool {
		*self == Self::default()
	}

	/// Whether the space is polar (cylindrical), making the hue direction option meaningful.
	pub fn is_polar(&self) -> bool {
		matches!(self, Self::OkLCh | Self::LCh | Self::Hsl | Self::Hsv)
	}

	// TODO: Remove when switching to the new document format and Ctrl-C node serialization format
	fn legacy_gamma() -> Self {
		Self::RgbGamma
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum GradientHueDirection {
	/// Interpolates across the shorter arc around the hue wheel.
	#[default]
	Shorter,
	/// Interpolates across the longer arc around the hue wheel.
	Longer,
	/// Interpolates with the hue angle always increasing.
	Increasing,
	/// Interpolates with the hue angle always decreasing.
	Decreasing,
}

impl GradientHueDirection {
	pub fn is_default(&self) -> bool {
		*self == Self::default()
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum GradientInterpolation {
	/// Holds each stop's color all the way to the next stop, jumping between them instead of transitioning.
	Stepped,
	/// Transitions straight from each stop to the next, turning a corner at every stop.
	#[default]
	Linear,
	/// Transitions along a curve that flows through the stops without corners.
	///
	/// The rate of color change carries smoothly through each stop (C1 continuity) and never overshoots beyond the stop colors, properties of its spline: a Piecewise Cubic Hermite Interpolating Polynomial (PCHIP) with Fritsch-Carlson tangent limiting.
	Smooth,
}

impl GradientInterpolation {
	pub fn is_default(&self) -> bool {
		*self == Self::default()
	}
}

/// The whole-ramp attributes governing how a gradient plays back, read together off a gradient item so the
/// sampling entry points take one argument instead of a widening list of same-typed positional ones.
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientSettings {
	pub spread: GradientSpread,
	pub cyclic: bool,
	pub space: GradientSpace,
	pub hue_direction: GradientHueDirection,
	pub interpolation: GradientInterpolation,
}

impl GradientSettings {
	/// The whole-ramp settings an item carries in its attributes beside a gradient element, defaulting each absent one.
	/// Generic over the element type so callers holding an unresolved item can still read them.
	pub fn from_item_attributes<T>(item: &Item<T>) -> Self {
		Self {
			spread: item.attribute_cloned_or_default(ATTR_GRADIENT_SPREAD),
			cyclic: item.attribute_cloned_or_default(ATTR_GRADIENT_CYCLIC),
			space: item.attribute_cloned_or_default(ATTR_GRADIENT_SPACE),
			hue_direction: item.attribute_cloned_or_default(ATTR_GRADIENT_HUE_DIRECTION),
			interpolation: item.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION),
		}
	}

	/// The whole-ramp settings a list carries at `index` beside a gradient element, defaulting each absent one.
	pub fn from_list_row_attributes<T>(list: &List<T>, index: usize) -> Self {
		Self {
			spread: list.attribute_cloned_or_default(ATTR_GRADIENT_SPREAD, index),
			cyclic: list.attribute_cloned_or_default(ATTR_GRADIENT_CYCLIC, index),
			space: list.attribute_cloned_or_default(ATTR_GRADIENT_SPACE, index),
			hue_direction: list.attribute_cloned_or_default(ATTR_GRADIENT_HUE_DIRECTION, index),
			interpolation: list.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION, index),
		}
	}
}

impl From<&Item<Gradient>> for GradientSettings {
	fn from(item: &Item<Gradient>) -> Self {
		Self::from_item_attributes(item)
	}
}

impl<C> From<&GradientRamp<C>> for GradientSettings {
	fn from(ramp: &GradientRamp<C>) -> Self {
		Self {
			spread: ramp.gradient_spread,
			cyclic: ramp.gradient_cyclic,
			space: ramp.gradient_space,
			hue_direction: ramp.gradient_hue_direction,
			interpolation: ramp.gradient_interpolation,
		}
	}
}

/// Rebuild the y-axis so its (parallel, perpendicular) components in the x-axis-aligned frame stay constant, both
/// rescaled by `|new_x| / |old_x|`. This holds the (x, y) parallelogram's aspect ratio and skew fixed across an endpoint
/// drag, so a radial ellipse stays the same shape (just rotated and resized) instead of distorting as x grows or shrinks.
/// Falls back to a +90° rotation of `new_x` when `old_x` is degenerate.
fn scale_y_axis_to_match_new_x(old_x: DVec2, old_y: DVec2, new_x: DVec2) -> DVec2 {
	let old_x_length = old_x.length();
	if old_x_length < 1e-9 {
		return DVec2::new(-new_x.y, new_x.x);
	}
	let ex_old = old_x / old_x_length;
	let ey_old = DVec2::new(-ex_old.y, ex_old.x);

	let new_x_length = new_x.length();
	if new_x_length < 1e-9 {
		return DVec2::ZERO;
	}
	let ex_new = new_x / new_x_length;
	let ey_new = DVec2::new(-ex_new.y, ex_new.x);

	let parallel = old_y.dot(ex_old);
	let perpendicular = old_y.dot(ey_old);
	let scale = new_x_length / old_x_length;

	scale * (parallel * ex_new + perpendicular * ey_new)
}

/// Build a new affine that maps canonical (0,0) -> (1,0) to (new_start, new_end), preserving the y-axis
/// shape of `old` proportionally to the x-axis length change.
pub fn build_transform_with_y_preservation(old: DAffine2, new_start: DVec2, new_end: DVec2) -> DAffine2 {
	let new_x_axis = new_end - new_start;
	let preserved_y_axis = scale_y_axis_to_match_new_x(old.matrix2.x_axis, old.matrix2.y_axis, new_x_axis);
	DAffine2 {
		matrix2: glam::DMat2::from_cols(new_x_axis, preserved_y_axis),
		translation: new_start,
	}
}

/// Build the default transform for a gradient not yet given one: a horizontal gradient spanning the
/// bounding box's width, running through its vertical middle.
pub fn initial_gradient_transform_for_bounding_box(bounds: [DVec2; 2]) -> DAffine2 {
	let [min, max] = bounds;
	let x_axis = DVec2::new(max.x - min.x, 0.);
	DAffine2 {
		matrix2: glam::DMat2::from_cols(x_axis, x_axis.perp()),
		translation: DVec2::new(min.x, (min.y + max.y) / 2.),
	}
}

impl core_types::bounds::BoundingBox for Gradient {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		core_types::bounds::RenderBoundingBox::Infinite
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		// AABB of the gradient line itself, leaving aspect padding and sub-pixel fallbacks to the runtime so this stays
		// a clean per-item geometric bound that combines naturally with siblings
		let start = transform.transform_point2(DVec2::ZERO);
		let end = transform.transform_point2(DVec2::X);
		core_types::bounds::RenderBoundingBox::Rectangle([start.min(end), start.max(end)])
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn default_is_empty_and_black_to_white_is_the_artist_starting_gradient() {
		assert!(Gradient::default().is_empty());
		assert_eq!(Gradient::black_to_white().positions(false), vec![0., 1.]);
		assert_eq!(Gradient::default().evaluate(0.5, Default::default()), Color::BLACK);
	}

	#[test]
	fn absent_attributes_default_to_even_positions_and_linear_midpoints() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		assert_eq!(gradient.positions(false), vec![0., 0.5, 1.]);
		assert_eq!(gradient.midpoints(), vec![0.5, 0.5, 0.5]);
	}

	#[test]
	fn serde_round_trip_preserves_attribute_absence() {
		let implicit = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		let json = serde_json::to_string(&implicit).unwrap();
		assert!(!json.contains("position") && !json.contains("midpoint"), "absent attributes must not serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), implicit);

		let mut explicit = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		explicit.set_positions(&[0.2, 0.9]);
		explicit.set_midpoints(&[0.3, 0.5]);
		let explicit = GradientRamp::from(explicit);
		let json = serde_json::to_string(&explicit).unwrap();
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), explicit);
	}

	#[test]
	fn gradient_spread_serializes_only_when_not_default() {
		let default_spread = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		let json = serde_json::to_string(&default_spread).unwrap();
		assert!(!json.contains("gradient_spread"), "the default Pad gradient spread must not serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), default_spread);

		let repeating = GradientRamp {
			gradient_spread: GradientSpread::Repeat,
			..default_spread.clone()
		};
		let json = serde_json::to_string(&repeating).unwrap();
		assert!(json.contains(r#""gradient_spread":"Repeat""#), "a non-default gradient spread must serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), repeating);
	}

	#[test]
	fn gradient_spread_round_trips_through_the_item_attribute() {
		let ramp = GradientRamp {
			gradient_spread: GradientSpread::Repeat,
			..GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]))
		};

		let item = Item::<Gradient>::from(ramp.clone());
		assert_eq!(
			item.attribute_cloned_or_default::<GradientSpread>(ATTR_GRADIENT_SPREAD),
			GradientSpread::Repeat,
			"the runtime item should carry the gradient spread as its attribute"
		);
		assert_eq!(GradientRamp::from(&item), ramp);

		let padded = Item::<Gradient>::from(GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE])));
		assert!(
			padded.attribute::<GradientSpread>(ATTR_GRADIENT_SPREAD).is_none(),
			"the default Pad must stay absent rather than materialize"
		);
	}

	#[test]
	fn gradient_space_always_serializes_and_its_absence_reads_as_legacy_gamma() {
		let default_space = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		let json = serde_json::to_string(&default_space).unwrap();
		assert!(
			json.contains(r#""gradient_space":"OkLab""#),
			"the space must serialize even at its default, marking the ramp as post-legacy: {json}"
		);
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), default_space);

		let gamma = GradientRamp {
			gradient_space: GradientSpace::RgbGamma,
			..default_space.clone()
		};
		let json = serde_json::to_string(&gamma).unwrap();
		assert!(json.contains(r#""gradient_space":"RgbGamma""#), "a non-default space must serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), gamma);

		let legacy_json = json.replace(r#","gradient_space":"RgbGamma""#, "");
		assert_eq!(
			serde_json::from_str::<GradientRamp>(&legacy_json).unwrap(),
			gamma,
			"a ramp saved before the field existed should read as the gamma it rendered with: {legacy_json}"
		);
	}

	#[test]
	fn gradient_space_round_trips_through_the_item_attribute() {
		let ramp = GradientRamp {
			gradient_space: GradientSpace::RgbGamma,
			..GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]))
		};

		let item = Item::<Gradient>::from(ramp.clone());
		assert_eq!(
			item.attribute_cloned_or_default::<GradientSpace>(ATTR_GRADIENT_SPACE),
			GradientSpace::RgbGamma,
			"the runtime item should carry the space as its attribute"
		);
		assert_eq!(GradientRamp::from(&item), ramp);

		let oklab = Item::<Gradient>::from(GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE])));
		assert!(
			oklab.attribute::<GradientSpace>(ATTR_GRADIENT_SPACE).is_none(),
			"the default OkLab must stay absent rather than materialize"
		);
	}

	#[test]
	fn gradient_hue_direction_round_trips_through_the_item_attribute() {
		let ramp = GradientRamp {
			gradient_hue_direction: GradientHueDirection::Longer,
			..GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]))
		};

		let item = Item::<Gradient>::from(ramp.clone());
		assert_eq!(
			item.attribute_cloned_or_default::<GradientHueDirection>(ATTR_GRADIENT_HUE_DIRECTION),
			GradientHueDirection::Longer,
			"the runtime item should carry the hue direction as its attribute"
		);
		assert_eq!(GradientRamp::from(&item), ramp);

		let shorter = Item::<Gradient>::from(GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE])));
		assert!(
			shorter.attribute::<GradientHueDirection>(ATTR_GRADIENT_HUE_DIRECTION).is_none(),
			"the default Shorter must stay absent rather than materialize"
		);
	}

	#[test]
	fn gradient_interpolation_round_trips_through_the_item_attribute() {
		let ramp = GradientRamp {
			gradient_interpolation: GradientInterpolation::Smooth,
			..GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]))
		};

		let item = Item::<Gradient>::from(ramp.clone());
		assert_eq!(
			item.attribute_cloned_or_default::<GradientInterpolation>(ATTR_GRADIENT_INTERPOLATION),
			GradientInterpolation::Smooth,
			"the runtime item should carry the interpolation as its attribute"
		);
		assert_eq!(GradientRamp::from(&item), ramp);

		let linear = Item::<Gradient>::from(GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE])));
		assert!(
			linear.attribute::<GradientInterpolation>(ATTR_GRADIENT_INTERPOLATION).is_none(),
			"the default Linear must stay absent rather than materialize"
		);
	}

	#[test]
	fn toggling_cyclic_leaves_the_stops_where_they_are() {
		// A ramp still riding the elided default is the case that would otherwise snap, since the two modes
		// spread the same stop count differently
		let elided = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]));
		assert_eq!(Gradient::from(&elided).positions(false), vec![0., 0.5, 1.]);

		let cyclic = elided.clone().with_cyclic(true);
		assert!(cyclic.gradient_cyclic);
		assert_eq!(Gradient::from(&cyclic).positions(true), vec![0., 0.5, 1.], "toggling cyclic on must not move the stops");

		// And back again, landing on the original elided form rather than the cyclic distribution
		let restored = cyclic.with_cyclic(false);
		assert_eq!(Gradient::from(&restored).positions(false), vec![0., 0.5, 1.], "toggling cyclic off must not move the stops");
		assert_eq!(restored, elided, "returning to the original mode should restore the canonical elided form");

		// A ramp already sitting on the cyclic even distribution keeps it, and elides once the flag agrees
		let mut thirds = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		thirds.set_positions(&[0., 1. / 3., 2. / 3.]);
		let ramp = GradientRamp::from(thirds).with_cyclic(true);
		assert_eq!(Gradient::from(&ramp).positions(true), vec![0., 1. / 3., 2. / 3.]);
		assert!(
			!Gradient::from(&ramp).has_position_attribute(),
			"matching the new mode's distribution should elide back to the canonical form"
		);
	}

	#[test]
	fn stepped_holds_each_color_to_the_next_stop_and_ignores_midpoints() {
		let stepped = GradientSettings {
			interpolation: GradientInterpolation::Stepped,
			..Default::default()
		};

		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		gradient.set_midpoints(&[0.1, 0.9, 0.5]);

		// Each stop's color fills its whole interval, jumping only once the next stop is reached
		for (t, expected) in [(0., Color::BLACK), (0.49, Color::BLACK), (0.5, Color::WHITE), (0.99, Color::WHITE), (1., Color::RED)] {
			assert_eq!(gradient.evaluate(t, stepped), expected, "stepped should hold the left stop's color at {t}");
		}

		// The bake reproduces the jumps exactly rather than approximating them, so each interval emits both of its ends
		let samples = gradient.interpolated_samples(stepped);
		assert_eq!(
			samples.iter().map(|&(position, color, _)| (position, color)).collect::<Vec<_>>(),
			vec![(0., Color::BLACK), (0.5, Color::BLACK), (0.5, Color::WHITE), (1., Color::WHITE), (1., Color::RED)]
		);
	}

	#[test]
	fn smooth_passes_through_every_stop_without_overshooting_between_them() {
		let smooth = GradientSettings {
			space: GradientSpace::RgbLinear,
			interpolation: GradientInterpolation::Smooth,
			..Default::default()
		};

		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::BLACK]);

		// A monotonic spline is pinned to its stops, unlike an overshooting one such as Catmull-Rom
		for (index, expected) in [(0., Color::BLACK), (0.5, Color::WHITE), (1., Color::BLACK)] {
			let sampled = gradient.evaluate(index, smooth);
			assert!((sampled.r() - expected.r()).abs() < 1e-4, "the spline must pass through the stop at {index}, got {sampled:?}");
		}

		// Every sample between two stops stays bounded by them, so no channel manufactures an out-of-gamut excursion
		for step in 0..=100 {
			let red = gradient.evaluate(step as f64 / 100., smooth).r();
			assert!((-1e-4..=1. + 1e-4).contains(&red), "the monotonic spline must not overshoot its stops, got {red} at step {step}");
		}
	}

	#[test]
	fn smooth_removes_the_rate_kink_that_linear_leaves_at_a_stop() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::from_rgbaf32_unchecked(0.25, 0.25, 0.25, 1.), Color::WHITE]);
		gradient.set_positions(&[0., 0.5, 1.]);

		// Sample the slope just either side of the middle stop, where Linear's independent chords change rate abruptly.
		// The step stays small so the one-sided differences approximate the slopes at the stop rather than averaging in curvature.
		let slope_around_middle = |settings: GradientSettings| {
			const STEP: f64 = 1e-3;
			let sample = |t: f64| gradient.evaluate(t, settings).r() as f64;
			let before = (sample(0.5) - sample(0.5 - STEP)) / STEP;
			let after = (sample(0.5 + STEP) - sample(0.5)) / STEP;
			(after - before).abs()
		};

		let linear = GradientSettings {
			space: GradientSpace::RgbLinear,
			..Default::default()
		};
		let smooth = GradientSettings {
			interpolation: GradientInterpolation::Smooth,
			..linear
		};

		assert!(
			slope_around_middle(smooth) < slope_around_middle(linear) / 10.,
			"smooth should carry its rate through the stop where linear turns a corner"
		);
	}

	#[test]
	fn smooth_closes_a_cyclic_loop_across_the_boundary() {
		let smooth_cyclic = GradientSettings {
			space: GradientSpace::RgbLinear,
			cyclic: true,
			interpolation: GradientInterpolation::Smooth,
			..Default::default()
		};

		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::from_rgbaf32_unchecked(0.5, 0.5, 0.5, 1.)]);

		// The two wrapped views of the boundary must agree, or the seam shows as a visible discontinuity
		let (before, after) = (gradient.evaluate(0.999, smooth_cyclic), gradient.evaluate(0.001, smooth_cyclic));
		assert!((before.r() - after.r()).abs() < 2. / 255., "the loop must join across the 1|0 boundary, got {before:?} then {after:?}");

		// The bake's ends share that crossing color exactly, so downstream renderers see a closed ramp
		let samples = gradient.interpolated_samples(smooth_cyclic);
		let (first, last) = (samples.first().expect("a baked ramp has samples"), samples.last().expect("a baked ramp has samples"));
		assert_eq!((first.0, first.1), (0., last.1), "the baked ends must share the boundary-crossing color");
		assert_eq!(last.0, 1.);
	}

	#[test]
	fn smooth_bake_keeps_the_end_stops_their_own_colors() {
		let smooth = GradientSettings {
			space: GradientSpace::RgbLinear,
			interpolation: GradientInterpolation::Smooth,
			..Default::default()
		};

		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		let samples = gradient.interpolated_samples(smooth);
		let (first, last) = (samples.first().expect("a baked ramp has samples"), samples.last().expect("a baked ramp has samples"));
		assert_eq!((first.0, first.1), (0., Color::BLACK));
		assert_eq!((last.0, last.1), (1., Color::WHITE));
	}

	#[test]
	fn smooth_cyclic_stops_on_both_boundaries_keep_a_hard_seam() {
		let open = GradientSettings {
			space: GradientSpace::RgbLinear,
			interpolation: GradientInterpolation::Smooth,
			..Default::default()
		};
		let cyclic = GradientSettings { cyclic: true, ..open };

		let mut gradient = Gradient::from(vec![Color::BLACK, Color::from_rgbaf32_unchecked(0.25, 0.25, 0.25, 1.), Color::WHITE]);
		gradient.set_positions(&[0., 0.5, 1.]);

		// With no wrapped interval to cross, the cycle's seam is the jump between the two end stops and the
		// spline matches its open form everywhere
		for step in 0..=100 {
			let t = step as f64 / 100.;
			assert_eq!(gradient.evaluate(t, cyclic), gradient.evaluate(t, open), "cyclic and open must agree at {t}");
		}

		let samples = gradient.interpolated_samples(cyclic);
		let (first, last) = (samples.first().expect("a baked ramp has samples"), samples.last().expect("a baked ramp has samples"));
		assert_eq!((first.0, first.1), (0., Color::BLACK));
		assert_eq!((last.0, last.1), (1., Color::WHITE));
	}

	#[test]
	fn linear_space_densifies_samples_where_gamma_segments_deviate() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		// Gamma needs no synthesized samples since the renderers already draw gamma segments
		assert_eq!(
			gradient
				.interpolated_samples(GradientSettings {
					space: GradientSpace::RgbGamma,
					..Default::default()
				})
				.len(),
			2
		);

		// A linear black-to-white ramp curves away from any single gamma segment, so samples must densify,
		// keeping the end stops in place and every synthesized color on the linear-light line
		let samples = gradient.interpolated_samples(GradientSettings {
			space: GradientSpace::RgbLinear,
			..Default::default()
		});
		assert!(samples.len() > 2, "the linear space should synthesize samples, got {}", samples.len());
		assert_eq!(samples.first().unwrap().0, 0.);
		assert_eq!(samples.last().unwrap().0, 1.);
		for &(position, color, _) in &samples {
			assert!(
				(color.r() as f64 - position).abs() < 1e-5,
				"sample at {position} should sit on the linear-light line, got {}",
				color.r()
			);
		}

		// Identical end colors leave nothing to densify
		let flat = Gradient::from(vec![Color::WHITE, Color::WHITE]);
		assert_eq!(
			flat.interpolated_samples(GradientSettings {
				space: GradientSpace::RgbLinear,
				..Default::default()
			})
			.len(),
			2
		);
	}

	#[test]
	fn midpoint_bias_and_gradient_space_compose_within_playback_tolerance() {
		let color_pairs = [
			(Color::BLACK, Color::WHITE),
			(Color::RED, Color::WHITE),
			(Color::from_rgbaf32_unchecked(0.9, 0.2, 0.05, 1.), Color::from_rgbaf32_unchecked(0.05, 0.3, 0.8, 0.5)),
		];

		// Sweep the midpoint against each space so its bias and the space's curvature also oppose each other,
		// asserting the emitted samples' gamma playback tracks the composed midpoint-then-space theoretical curve
		for (gradient_space, gradient_hue_direction) in [
			(GradientSpace::OkLab, GradientHueDirection::Shorter),
			(GradientSpace::OkLCh, GradientHueDirection::Shorter),
			(GradientSpace::OkLCh, GradientHueDirection::Longer),
			(GradientSpace::Lab, GradientHueDirection::Shorter),
			(GradientSpace::LCh, GradientHueDirection::Shorter),
			(GradientSpace::Hsl, GradientHueDirection::Shorter),
			(GradientSpace::Hsl, GradientHueDirection::Longer),
			(GradientSpace::Hsv, GradientHueDirection::Shorter),
			(GradientSpace::RgbLinear, GradientHueDirection::Shorter),
			(GradientSpace::RgbGamma, GradientHueDirection::Shorter),
		] {
			for &(color_a, color_b) in &color_pairs {
				for midpoint_step in 1..40 {
					let midpoint = midpoint_step as f64 / 40.;

					let mut gradient = Gradient::from(vec![color_a, color_b]);
					gradient.set_midpoints(&[midpoint, 0.5]);
					let samples = gradient.interpolated_samples(GradientSettings {
						space: gradient_space,
						hue_direction: gradient_hue_direction,
						..Default::default()
					});

					for probe in 0..=1000 {
						let t = probe as f64 / 1000.;

						let after = samples.iter().position(|&(position, ..)| position >= t).unwrap_or(samples.len() - 1);
						let playback = if after == 0 {
							samples[0].1
						} else {
							let (left_position, left_color, _) = samples[after - 1];
							let (right_position, right_color, _) = samples[after];
							let span = right_position - left_position;
							if span < 1e-12 {
								right_color
							} else {
								left_color.lerp_gamma_srgb(&right_color, ((t - left_position) / span) as f32)
							}
						};

						let true_color = interpolate_stop_colors(color_a, color_b, apply_midpoint(t, midpoint) as f32, gradient_space, gradient_hue_direction);
						let deviation = max_gamma_channel_deviation(playback, true_color);
						assert!(
							deviation <= 4. / 255.,
							"playback deviates {:.1}/255 at t={t} with midpoint {midpoint} in {gradient_space:?} ({gradient_hue_direction:?}) between {color_a:?} and {color_b:?}",
							deviation * 255.
						);
					}
				}
			}
		}
	}

	#[test]
	fn svg_background_image_percent_encodes_and_keeps_straight_alpha_stops() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		gradient.set_color(1, Color::from_rgbaf32_unchecked(1., 1., 1., 0.5));

		let image = gradient.to_svg_background_image(GradientSettings::default());

		assert!(image.starts_with("url(\"data:image/svg+xml,"), "the value should be an SVG data URI: {image}");
		assert!(image.contains("stop-opacity='0.5'"), "a transparent stop should emit its straight alpha: {image}");
		assert!(!image.contains(['#', '<', '>']), "URI-hostile characters should be percent-encoded: {image}");
	}

	#[test]
	fn svg_background_image_paints_a_stopless_gradient_black() {
		let image = Gradient::from(Vec::new()).to_svg_background_image(GradientSettings::default());

		// The hex color's `#` arrives percent-encoded
		assert!(image.contains("stop-color='%23000000'"), "a gradient with no stops should paint black rather than nothing: {image}");
	}

	#[test]
	fn clear_spread_evaluates_to_transparency_outside_the_unit_range() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		assert_eq!(
			gradient.evaluate(
				-0.25,
				GradientSettings {
					spread: GradientSpread::Clear,
					..Default::default()
				}
			),
			Color::TRANSPARENT
		);
		assert_eq!(
			gradient.evaluate(
				1.25,
				GradientSettings {
					spread: GradientSpread::Clear,
					..Default::default()
				}
			),
			Color::TRANSPARENT
		);

		for t in [0., 0.25, 1.] {
			assert_eq!(
				gradient.evaluate(
					t,
					GradientSettings {
						spread: GradientSpread::Clear,
						..Default::default()
					}
				),
				gradient.evaluate(
					t,
					GradientSettings {
						spread: GradientSpread::Pad,
						..Default::default()
					}
				),
				"inside the range Clear must match Pad at t = {t}"
			);
		}
	}

	#[test]
	fn evaluate_follows_the_gradient_space() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		let oklab = gradient.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::OkLab,
				..Default::default()
			},
		);
		let linear = gradient.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::RgbLinear,
				..Default::default()
			},
		);
		let gamma = gradient.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::RgbGamma,
				..Default::default()
			},
		);

		assert_eq!(linear, Color::BLACK.lerp(&Color::WHITE, 0.5));
		assert_eq!(gamma, Color::BLACK.lerp_gamma_srgb(&Color::WHITE, 0.5));

		// Halfway in OkLab between black and white is lightness 0.5, which cubes to 1/8 linear light in every channel
		for channel in [oklab.r(), oklab.g(), oklab.b()] {
			assert!((channel - 0.125).abs() < 1e-3, "the OkLab mid color of black and white should be 1/8 linear light, got {channel}");
		}
		assert_eq!(oklab.a(), 1.);

		assert_ne!(linear, gamma, "each space must produce a different mid color between black and white");
		assert_ne!(oklab, linear, "each space must produce a different mid color between black and white");
		assert_ne!(oklab, gamma, "each space must produce a different mid color between black and white");
	}

	#[test]
	fn polar_spaces_take_the_shorter_hue_arc_and_ignore_powerless_hues() {
		use color::ColorSpace;

		// Red to blue in HSL crosses through magenta on the shorter arc (300 degrees), not through green (120 degrees)
		let red_to_blue = Gradient::from(vec![Color::RED, Color::BLUE]);
		let magenta = red_to_blue.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::Hsl,
				..Default::default()
			},
		);
		for (channel, expected) in [(magenta.r(), 1.), (magenta.g(), 0.), (magenta.b(), 1.)] {
			assert!((channel - expected).abs() < 1e-3, "the HSL mid color of red and blue should be magenta, got {magenta:?}");
		}

		// White's hue is powerless, so an OkLCh interpolation toward it keeps red's hue instead of drifting toward white's arbitrary hue
		let red_to_white = Gradient::from(vec![Color::RED, Color::WHITE]);
		let pink = red_to_white.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::OkLCh,
				..Default::default()
			},
		);
		let [_, _, red_hue] = color::Oklch::from_linear_srgb([Color::RED.r(), Color::RED.g(), Color::RED.b()]);
		let [_, pink_chroma, pink_hue] = color::Oklch::from_linear_srgb([pink.r(), pink.g(), pink.b()]);
		assert!(pink_chroma > 0.05, "the mid color should stay chromatic, got {pink:?}");
		assert!((pink_hue - red_hue).abs() < 0.5, "the mid hue should hold red's {red_hue} degrees, got {pink_hue}");

		// HSV rides the cube's top face toward white, keeping the mid tint at full brightness where HSL dips
		let tint = red_to_white.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::Hsv,
				..Default::default()
			},
		);
		for (channel, target) in tint.to_gamma_srgb_channels().into_iter().zip([1., 0.5, 0.5, 1.]) {
			assert!((channel - target).abs() < 1e-3, "the HSV mid tint of red and white should be gamma (1, 0.5, 0.5), got {tint:?}");
		}

		// Toward black both saturation and value halve, the classic HSV shade that neither HSL nor HWB produces
		let red_to_black = Gradient::from(vec![Color::RED, Color::BLACK]);
		let shade = red_to_black.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::Hsv,
				..Default::default()
			},
		);
		for (channel, target) in shade.to_gamma_srgb_channels().into_iter().zip([0.5, 0.25, 0.25, 1.]) {
			assert!((channel - target).abs() < 1e-3, "the HSV mid shade of red and black should be gamma (0.5, 0.25, 0.25), got {shade:?}");
		}
	}

	#[test]
	fn hue_direction_chooses_the_arc_around_the_hue_wheel() {
		// Red to blue spans 240 degrees upward, so Longer and Increasing agree on the green route
		// while Shorter and Decreasing cross through magenta
		let red_to_blue = Gradient::from(vec![Color::RED, Color::BLUE]);
		let expectations = [
			(GradientHueDirection::Shorter, [1., 0., 1.]),
			(GradientHueDirection::Longer, [0., 1., 0.]),
			(GradientHueDirection::Increasing, [0., 1., 0.]),
			(GradientHueDirection::Decreasing, [1., 0., 1.]),
		];
		for (gradient_hue_direction, expected_rgb) in expectations {
			let mid = red_to_blue.evaluate(
				0.5,
				GradientSettings {
					space: GradientSpace::Hsl,
					hue_direction: gradient_hue_direction,
					..Default::default()
				},
			);
			for (channel, target) in [mid.r(), mid.g(), mid.b()].into_iter().zip(expected_rgb) {
				assert!(
					(channel - target).abs() < 1e-3,
					"the {gradient_hue_direction:?} mid of red and blue should be {expected_rgb:?}, got {mid:?}"
				);
			}
		}

		// Identical hues under Longer take a full turn around the wheel, passing through cyan halfway
		let red_to_red = Gradient::from(vec![Color::RED, Color::RED]);
		let mid = red_to_red.evaluate(
			0.5,
			GradientSettings {
				space: GradientSpace::Hsl,
				hue_direction: GradientHueDirection::Longer,
				..Default::default()
			},
		);
		for (channel, target) in [mid.r(), mid.g(), mid.b()].into_iter().zip([0., 1., 1.]) {
			assert!((channel - target).abs() < 1e-3, "the full-turn mid of red and red should be cyan, got {mid:?}");
		}
	}

	#[test]
	fn gradient_ui_write_back_elides_default_attributes() {
		// The picker always sends explicit positions, so the write-back is what restores the canonical absence-as-default form
		let write_back = |gradient: &Gradient, gradient_cyclic: bool| {
			let sent = GradientRamp::<SRGBA8> { gradient_cyclic, ..gradient.into() };
			Gradient::from(&GradientRamp::from(&sent))
		};

		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		gradient.set_positions(&[0., 0.5, 1.]);
		gradient.set_midpoints(&[0.7, 0.5, 0.5]);

		let written_back = write_back(&gradient, false);
		assert!(!written_back.has_position_attribute(), "the even distribution should elide on write-back");
		assert_eq!(written_back.midpoints(), vec![0.7, 0.5, 0.5]);

		// Cyclic ramps spread over one more interval, so thirds are the elidable distribution and halves are not
		let mut cyclic = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		cyclic.set_positions(&[0., 1. / 3., 2. / 3.]);
		assert!(!write_back(&cyclic, true).has_position_attribute(), "the cyclic even distribution should elide on write-back");
		assert_eq!(
			write_back(&gradient, true).positions(true),
			vec![0., 0.5, 1.],
			"positions that only look default when non-cyclic must survive"
		);
	}

	#[test]
	fn nondefault_attributes_elide_default_values() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		assert_eq!(gradient.nondefault_positions(false), None);
		assert_eq!(gradient.nondefault_midpoints(), None);

		// Explicit attributes that merely restate the defaults still elide
		gradient.set_positions(&[0., 0.5, 1.]);
		gradient.set_midpoints(&[0.5, 0.5, 0.5]);
		assert_eq!(gradient.nondefault_positions(false), None);
		assert_eq!(gradient.nondefault_midpoints(), None);

		gradient.set_positions(&[0., 0.25, 1.]);
		gradient.set_midpoints(&[0.5, 0.7, 0.5]);
		assert_eq!(gradient.nondefault_positions(false), Some(vec![0., 0.25, 1.]));
		assert_eq!(gradient.nondefault_midpoints(), Some(vec![0.5, 0.7, 0.5]));
	}

	#[test]
	fn non_compliant_positions_normalize_for_sampling_and_rendering() {
		// Stored positions stay as authored and consumers sort them, with the bake clipped to the range the renderers accept
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK, Color::RED]);
		gradient.set_positions(&[1.5, 0.4, -0.5]);
		assert_eq!(gradient.positions(false), vec![1.5, 0.4, -0.5]);

		let settings = GradientSettings {
			space: GradientSpace::RgbGamma,
			..Default::default()
		};

		let sample_positions: Vec<f64> = gradient.interpolated_samples(settings).iter().map(|(position, ..)| *position).collect();
		assert!(sample_positions.windows(2).all(|pair| pair[0] <= pair[1]), "samples must ascend: {sample_positions:?}");
		assert_eq!(sample_positions.first(), Some(&0.));
		assert_eq!(sample_positions.last(), Some(&1.));

		// The outermost stops lie beyond the range, so the ends interpolate toward them instead of adopting their colors
		let start = gradient.evaluate(0., settings);
		let end = gradient.evaluate(1., settings);
		assert!(max_gamma_channel_deviation(start, Color::RED.lerp_gamma_srgb(&Color::BLACK, 0.5 / 0.9)) < 1e-6, "got {start:?}");
		assert!(max_gamma_channel_deviation(end, Color::BLACK.lerp_gamma_srgb(&Color::WHITE, 0.6 / 1.1)) < 1e-6, "got {end:?}");
	}

	#[test]
	fn infinite_positions_clamp_to_the_range_ends() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[f64::INFINITY, f64::NEG_INFINITY]);

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(GradientSettings {
				space: GradientSpace::RgbGamma,
				..Default::default()
			})
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(gradient.evaluate(0., Default::default()), Color::BLACK);
		assert_eq!(gradient.evaluate(1., Default::default()), Color::WHITE);
	}

	#[test]
	fn nan_positions_drop_their_stops_from_sampling() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK, Color::RED]);
		gradient.set_positions(&[0., f64::NAN, 1.]);

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(GradientSettings {
				space: GradientSpace::RgbGamma,
				..Default::default()
			})
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(
			gradient.evaluate(
				0.5,
				GradientSettings {
					space: GradientSpace::RgbLinear,
					..Default::default()
				}
			),
			Color::WHITE.lerp(&Color::RED, 0.5)
		);

		// A non-finite position is preserved as nondefault so write-back elision cannot resurrect the dropped stop
		assert!(gradient.nondefault_positions(false).is_some());

		// With every position NaN the gradient samples as stopless, painting solid black to signal the upstream bug
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::RED]);
		gradient.set_positions(&[f64::NAN, f64::NAN]);
		assert!(
			gradient
				.interpolated_samples(GradientSettings {
					space: GradientSpace::RgbGamma,
					..Default::default()
				})
				.is_empty()
		);
		assert_eq!(gradient.evaluate(0.5, Default::default()), Color::BLACK);
	}

	#[test]
	fn samples_start_at_the_first_stop_without_synthetic_lead_in() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[0.3, 1.]);

		let samples = gradient.interpolated_samples(GradientSettings {
			space: GradientSpace::RgbGamma,
			..Default::default()
		});
		assert_eq!(samples[0], (0.3, Color::WHITE, None), "renderers that need a flat lead-in before the first stop add it themselves");
	}

	#[test]
	fn nan_midpoints_read_as_linear() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		let linear_result = gradient.evaluate(0.25, Default::default());

		gradient.set_midpoints(&[f64::NAN, f64::NAN]);
		assert_eq!(gradient.evaluate(0.25, Default::default()), linear_result);
		let no_nan_annotations = gradient
			.interpolated_samples(GradientSettings {
				space: GradientSpace::RgbGamma,
				..Default::default()
			})
			.iter()
			.all(|(position, _, midpoint)| position.is_finite() && !midpoint.is_some_and(|midpoint| midpoint.is_nan()));
		assert!(no_nan_annotations, "NaN must not escape into rendered sample annotations");
	}

	#[test]
	fn gradient_cyclic_serializes_only_when_set_and_rides_the_item_attribute() {
		let ramp = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		let json = serde_json::to_string(&ramp).unwrap();
		assert!(!json.contains("gradient_cyclic"), "the default non-cyclic flag must not serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), ramp);

		let cyclic = GradientRamp { gradient_cyclic: true, ..ramp };
		let json = serde_json::to_string(&cyclic).unwrap();
		assert!(json.contains(r#""gradient_cyclic":true"#), "an enabled cyclic flag must serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), cyclic);

		let item = Item::<Gradient>::from(cyclic.clone());
		assert!(
			item.attribute_cloned_or_default::<bool>(ATTR_GRADIENT_CYCLIC),
			"the runtime item should carry the cyclic flag as its attribute"
		);
		assert_eq!(GradientRamp::from(&item), cyclic);
	}

	#[test]
	fn cyclic_reserves_the_wrapped_interval_in_the_even_distribution() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED, Color::BLUE]);
		assert_eq!(gradient.positions(false), vec![0., 1. / 3., 2. / 3., 1.]);
		assert_eq!(gradient.positions(true), vec![0., 0.25, 0.5, 0.75]);
	}

	#[test]
	fn cyclic_evaluate_wraps_from_the_last_stop_back_to_the_first() {
		// Elided cyclic positions put the stops at 0 and 0.5, so the wrapped interval spans the other half
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		let quarter = gradient.evaluate(
			0.25,
			GradientSettings {
				cyclic: true,
				space: GradientSpace::RgbLinear,
				..Default::default()
			},
		);
		let wrap_quarter = gradient.evaluate(
			0.75,
			GradientSettings {
				cyclic: true,
				space: GradientSpace::RgbLinear,
				..Default::default()
			},
		);
		assert_eq!(quarter, Color::BLACK.lerp(&Color::WHITE, 0.5));
		assert_eq!(wrap_quarter, Color::WHITE.lerp(&Color::BLACK, 0.5));

		// A wrapped interval crossing the 1|0 boundary reads as one continuous span, so its two sides agree at the seam
		let mut offset = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		offset.set_positions(&[0.25, 0.5]);
		let at_end = offset.evaluate(
			1.,
			GradientSettings {
				cyclic: true,
				space: GradientSpace::RgbLinear,
				..Default::default()
			},
		);
		let at_start = offset.evaluate(
			0.,
			GradientSettings {
				cyclic: true,
				space: GradientSpace::RgbLinear,
				..Default::default()
			},
		);
		assert_eq!(at_end, at_start, "the 1|0 boundary must be seamless");
		assert_eq!(at_end, Color::WHITE.lerp(&Color::BLACK, 2. / 3.));
	}

	#[test]
	fn cyclic_wrapped_interval_times_with_the_last_stops_midpoint() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		gradient.set_midpoints(&[0.5, 0.25]);

		let expected_t = apply_midpoint(0.5, 0.25);
		let mid = gradient.evaluate(
			0.75,
			GradientSettings {
				cyclic: true,
				space: GradientSpace::RgbLinear,
				..Default::default()
			},
		);
		assert_eq!(mid, Color::WHITE.lerp(&Color::BLACK, expected_t as f32));
	}

	#[test]
	fn cyclic_samples_bake_the_wrap_and_play_back_within_tolerance() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::WHITE]);
		gradient.set_positions(&[0.25, 0.5]);
		gradient.set_midpoints(&[0.5, 0.3]);

		let samples = gradient.interpolated_samples(GradientSettings {
			cyclic: true,
			space: GradientSpace::OkLab,
			..Default::default()
		});
		assert_eq!(samples.first().unwrap().0, 0.);
		assert_eq!(samples.last().unwrap().0, 1.);
		assert_eq!(samples.first().unwrap().1, samples.last().unwrap().1, "both ends must share the boundary-crossing color");
		assert!(samples.windows(2).all(|pair| pair[0].0 <= pair[1].0), "samples must ascend");

		// The flat samples' gamma playback must track the true cyclic curve, wrapped interval included
		for probe in 0..=400 {
			let t = probe as f64 / 400.;

			let after = samples.iter().position(|&(position, ..)| position >= t).unwrap_or(samples.len() - 1);
			let playback = if after == 0 {
				samples[0].1
			} else {
				let (left_position, left_color, _) = samples[after - 1];
				let (right_position, right_color, _) = samples[after];
				let span = right_position - left_position;
				if span < 1e-12 {
					right_color
				} else {
					left_color.lerp_gamma_srgb(&right_color, ((t - left_position) / span) as f32)
				}
			};

			let true_color = gradient.evaluate(
				t,
				GradientSettings {
					cyclic: true,
					space: GradientSpace::OkLab,
					..Default::default()
				},
			);
			let deviation = max_gamma_channel_deviation(playback, true_color);
			assert!(deviation <= 4. / 255., "playback deviates {:.1}/255 at t={t}", deviation * 255.);
		}
	}

	#[test]
	fn cyclic_even_positions_elide_against_their_own_distribution() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		gradient.set_positions(&[0., 1. / 3., 2. / 3.]);
		assert!(gradient.nondefault_positions(true).is_none());
		assert!(gradient.nondefault_positions(false).is_some());

		gradient.elide_default_attributes(true);
		assert!(!gradient.has_position_attribute(), "cyclic-even positions should elide under the cyclic default");
	}

	#[test]
	fn inserting_into_the_wrapped_interval_inherits_the_wrap_handle_and_samples_its_color() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		gradient.set_midpoints(&[0.5, 0.3]);

		// The elided cyclic stops sit at 0 and 0.5, so 0.75 lands mid-wrap
		let index = gradient.insert_stop(
			0.75,
			GradientSettings {
				cyclic: true,
				space: GradientSpace::RgbLinear,
				..Default::default()
			},
		);
		assert_eq!(index, 2);
		assert_eq!(gradient.midpoint(2), 0.3, "the wrap handle should be inherited by the split");
		assert_eq!(gradient.color(2), Some(Color::WHITE.lerp(&Color::BLACK, apply_midpoint(0.5, 0.3) as f32)));
	}

	#[test]
	fn cyclic_reversal_materializes_the_even_distribution_and_flips_the_wrap_handle() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		gradient.set_midpoints(&[0.5, 0.5, 0.25]);

		let reversed = gradient.reversed(true);
		let positions = reversed.positions(true);
		for (actual, expected) in positions.iter().zip([1. / 3., 2. / 3., 1.]) {
			assert!((actual - expected).abs() < 1e-12, "reversed positions should be thirds ending at 1, got {positions:?}");
		}
		assert_eq!(
			reversed.midpoints(),
			vec![0.5, 0.5, 0.75],
			"the wrap handle should flip in place while interval midpoints shift and flip"
		);
	}

	#[test]
	fn shift_rotates_a_cyclic_ramp_back_onto_the_even_distribution() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE, Color::WHITE]);
		assert!(!gradient.has_position_attribute(), "an even distribution starts elided");

		gradient.shift_positions(0.25, true);

		let colors: Vec<Color> = (0..gradient.len()).filter_map(|index| gradient.color(index)).collect();
		assert_eq!(colors, vec![Color::WHITE, Color::RED, Color::GREEN, Color::BLUE], "a quarter turn should rotate the stops by one");
		assert!(!gradient.has_position_attribute(), "landing back on the even distribution should re-elide the positions");
	}

	#[test]
	fn cyclic_shift_keeps_stops_at_both_ends_together_and_in_order() {
		// Stops at 0 and 1 occupy one point on the circle, so every shift has to place them together and break the
		// resulting tie the same way, or the render flickers as the fraction sweeps
		let mut gradient = Gradient::from(vec![Color::RED, Color::BLUE]);
		gradient.set_positions(&[0., 1.]);

		// Decimal steps, as a slider produces them, since a dyadic fraction would wrap exactly either way
		for step in 1..100 {
			let mut shifted = gradient.clone();
			shifted.shift_positions(step as f64 * 0.01, true);

			let positions = shifted.positions(true);
			assert_eq!(positions[0], positions[1], "both ends should land on an identical position, got {positions:?}");
			assert_eq!(shifted.color(0), Some(Color::RED), "the tie should keep the original stop order");
		}
	}

	#[test]
	fn stops_shifted_out_of_range_sample_through_instead_of_piling_at_the_edge() {
		let settings = GradientSettings {
			space: GradientSpace::RgbGamma,
			..Default::default()
		};

		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);
		gradient.shift_positions(-0.6, false);

		// The red stop slid to -0.6 and the green to -0.1, so the range's start lands a fifth of the way from green to blue
		let expected = Color::GREEN.lerp_gamma_srgb(&Color::BLUE, 0.2);
		let edge = gradient.evaluate(0., settings);
		assert!(
			max_gamma_channel_deviation(edge, expected) < 1e-6,
			"the start should interpolate through the stops that slid out, got {edge:?}"
		);

		let samples = gradient.interpolated_samples(settings);
		assert!(
			samples.iter().all(|&(position, ..)| (0. ..=1.).contains(&position)),
			"the bake should carry no offsets outside the range"
		);
		let &(first_position, first_color, _) = samples.first().expect("a three stop ramp bakes samples");
		assert_eq!(first_position, 0.);
		assert!(
			max_gamma_channel_deviation(first_color, expected) < SAMPLE_THRESHOLD,
			"the baked start should match the sampled start rather than the stop that slid out, got {first_color:?}"
		);
	}

	#[test]
	fn shift_translates_a_non_cyclic_ramp_and_stays_reversible() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);

		gradient.shift_positions(0.25, false);
		assert_eq!(gradient.positions(false), vec![0.25, 0.75, 1.25], "stops should slide past the end rather than wrapping to the front");
		let colors: Vec<Color> = (0..gradient.len()).filter_map(|index| gradient.color(index)).collect();
		assert_eq!(colors, vec![Color::RED, Color::GREEN, Color::BLUE], "a translation should leave the stop order alone");

		gradient.shift_positions(-0.25, false);
		assert_eq!(
			gradient.positions(false),
			vec![0., 0.5, 1.],
			"the out-of-range position should survive so shifting back restores the ramp"
		);
		assert!(!gradient.has_position_attribute(), "landing back on the even distribution should re-elide the positions");
	}

	#[test]
	fn shift_carries_midpoints_with_their_stops_and_ignores_whole_turns() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE, Color::WHITE]);
		gradient.set_midpoints(&[0.8, 0.5, 0.5, 0.5]);

		let untouched = gradient.clone();
		gradient.shift_positions(1., true);
		assert_eq!(gradient, untouched, "a whole turn should leave the ramp exactly as it was");

		gradient.shift_positions(0.25, true);
		assert_eq!(gradient.color(1), Some(Color::RED));
		assert_eq!(gradient.midpoints(), vec![0.5, 0.8, 0.5, 0.5], "red's midpoint should follow it to its new index");
	}

	#[test]
	fn stretch_spreads_stops_about_the_pivot_and_stays_reversible() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);

		gradient.stretch_positions(2., 0.5, false);
		assert_eq!(gradient.positions(false), vec![-0.5, 0.5, 1.5], "the stop on the pivot should stay put while the rest spread out");
		let colors: Vec<Color> = (0..gradient.len()).filter_map(|index| gradient.color(index)).collect();
		assert_eq!(colors, vec![Color::RED, Color::GREEN, Color::BLUE], "a positive factor should leave the stop order alone");

		gradient.stretch_positions(0.5, 0.5, false);
		assert_eq!(
			gradient.positions(false),
			vec![0., 0.5, 1.],
			"the out-of-range positions should survive so the reciprocal restores the ramp"
		);
		assert!(!gradient.has_position_attribute(), "landing back on the even distribution should re-elide the positions");
	}

	#[test]
	fn negative_stretch_matches_reversal_when_mirrored_about_the_middle() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);
		gradient.set_midpoints(&[0.25, 0.125, 0.5]);

		let mut mirrored = gradient.clone();
		mirrored.stretch_positions(-1., 0.5, false);

		assert_eq!(mirrored, gradient.reversed(false), "mirroring about the middle is exactly a reversal");
		assert_eq!(
			mirrored.midpoints(),
			vec![0.875, 0.75, 0.5],
			"each interval's midpoint should flip and move to the stop now preceding it"
		);
	}

	#[test]
	fn negative_stretch_mirrors_across_an_off_center_pivot() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);

		gradient.stretch_positions(-1., 0.25, false);

		let colors: Vec<Color> = (0..gradient.len()).filter_map(|index| gradient.color(index)).collect();
		assert_eq!(colors, vec![Color::BLUE, Color::GREEN, Color::RED], "mirroring should reverse the stop order");
		assert_eq!(
			gradient.positions(false),
			vec![-0.5, 0., 0.5],
			"each stop should land as far from the pivot as it started, on the other side"
		);
	}

	#[test]
	fn cyclic_stretch_mirrors_around_the_circle_and_re_elides() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE, Color::WHITE]);
		assert!(!gradient.has_position_attribute(), "an even distribution starts elided");

		// Mirroring about the ramp's start sends every other stop the long way around the loop
		gradient.stretch_positions(-1., 0., true);

		let colors: Vec<Color> = (0..gradient.len()).filter_map(|index| gradient.color(index)).collect();
		assert_eq!(
			colors,
			vec![Color::RED, Color::WHITE, Color::BLUE, Color::GREEN],
			"the stop on the pivot should stay while the rest wrap past it"
		);
		assert!(!gradient.has_position_attribute(), "landing back on the even distribution should re-elide the positions");
	}

	#[test]
	fn a_lone_stop_shifted_out_of_range_still_bakes_inside_it() {
		let mut gradient = Gradient::from(vec![Color::RED]);
		gradient.shift_positions(2., false);

		let samples = gradient.interpolated_samples(Default::default());
		let positions: Vec<f64> = samples.iter().map(|(position, ..)| *position).collect();
		assert!(
			positions.iter().all(|position| (0. ..=1.).contains(position)),
			"renderers only accept offsets from 0 to 1, got {positions:?}"
		);
		assert_eq!(samples.first().map(|(_, color, _)| *color), Some(Color::RED), "the lone stop's color should still fill the ramp");
	}
}
