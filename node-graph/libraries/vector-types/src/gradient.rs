use core_types::Color;
use core_types::color::SRGBA8;
use core_types::list::{ATTR_GRADIENT_CYCLIC, ATTR_GRADIENT_HUE_DIRECTION, ATTR_GRADIENT_SPACE, ATTR_GRADIENT_SPREAD, ATTR_MIDPOINT, ATTR_POSITION, Item, List};
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
pub struct Gradient(List<Color>);

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

// Color picker round-trip: the caller should follow with `elide_default_attributes` (which needs the ramp's cyclic
// flag to know the default distribution) to restore the canonical absence-as-default form
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
	/// CSS `linear-gradient(...)` string. Stops are emitted as `#rrggbbaa` hex (already gamma-encoded bytes).
	pub fn to_css_linear_gradient(&self, gradient_cyclic: bool, gradient_space: GradientSpace, gradient_hue_direction: GradientHueDirection) -> String {
		Gradient::from(self).to_css_linear_gradient(gradient_cyclic, gradient_space, gradient_hue_direction)
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
	// TODO: Elide the default again (removing `legacy_gamma` and the serde aliases) when switching to the new document format and Ctrl-C node serialization format
	#[cfg_attr(feature = "serde", serde(default = "GradientSpace::legacy_gamma", alias = "gradient_interpolation"))]
	pub gradient_space: GradientSpace,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_cyclic: bool,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientHueDirection::is_default"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_hue_direction: GradientHueDirection,
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

// Color picker round-trip: routes through the runtime type so default-restating attributes elide
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
		}
	}
}

impl From<&GradientRamp<SRGBA8>> for GradientRamp {
	fn from(ramp: &GradientRamp<SRGBA8>) -> Self {
		Self {
			gradient_spread: ramp.gradient_spread,
			gradient_space: ramp.gradient_space,
			gradient_cyclic: ramp.gradient_cyclic,
			gradient_hue_direction: ramp.gradient_hue_direction,
			..Self::from(&ramp.stops)
		}
	}
}

impl GradientRamp {
	pub fn black_to_white() -> Self {
		Self::from(Gradient::black_to_white())
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
	use color::ColorSpaceLayout;

	let mut a = CS::from_linear_srgb([color_a.r(), color_a.g(), color_a.b()]);
	let mut b = CS::from_linear_srgb([color_b.r(), color_b.g(), color_b.b()]);

	let hue_index = match CS::LAYOUT {
		ColorSpaceLayout::HueFirst => Some(0),
		ColorSpaceLayout::HueThird => Some(2),
		_ => None,
	};
	if let Some(hue_index) = hue_index {
		// Chroma (or saturation) is channel 1 in both polar layouts; the threshold scales to the space's
		// lightness range so conversion noise on achromatic colors stays below it
		let achromatic = 1e-4 * CS::WHITE_COMPONENTS.iter().fold(0_f32, |max, &component| max.max(component));
		if a[1] < achromatic && b[1] >= achromatic {
			a[hue_index] = b[hue_index];
		}
		if b[1] < achromatic && a[1] >= achromatic {
			b[hue_index] = a[hue_index];
		}

		// The CSS Color 4 hue fixup, on hues the conversions already place in the 0 to 360 range
		let delta = b[hue_index] - a[hue_index];
		let delta = match gradient_hue_direction {
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
		};
		b[hue_index] = a[hue_index] + delta;
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
	pub fn insert_stop(&mut self, position: f64, gradient_cyclic: bool, gradient_space: GradientSpace, gradient_hue_direction: GradientHueDirection) -> usize {
		let color = self.evaluate(position, Default::default(), gradient_cyclic, gradient_space, gradient_hue_direction);
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

	/// Gradient stops as evaluation and rendering should see them: positions clamped to the 0 to 1 range
	/// (infinities landing at the ends, a NaN dropping its stop from sampling since it has no defined placement)
	/// and sorted ascending, so the sampler and every renderer agree on how non-compliant authored data behaves.
	fn normalized_stops(&self, gradient_cyclic: bool) -> Vec<GradientStop> {
		let mut stops: Vec<GradientStop> = (0..self.len())
			.filter_map(|index| {
				let position = self.position(index, gradient_cyclic).clamp(0., 1.);
				if position.is_nan() {
					return None;
				}

				let midpoint = self.midpoint(index);
				let color = self.color(index)?;

				Some(GradientStop { position, midpoint, color })
			})
			.collect();

		stops.sort_by(|a, b| a.position.total_cmp(&b.position));
		stops
	}

	/// Samples the gradient's color at `t`. Given a `t` outside the 0 to 1 range, the `gradient_spread` determines how the gradient extends.
	pub fn evaluate(&self, t: f64, gradient_spread: GradientSpread, gradient_cyclic: bool, gradient_space: GradientSpace, gradient_hue_direction: GradientHueDirection) -> Color {
		let t = match gradient_spread {
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

		let stops = self.normalized_stops(gradient_cyclic);
		let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return Color::BLACK };

		if gradient_cyclic && (t < first.position || t > last.position) {
			let wrap_length = first.position + 1. - last.position;
			if wrap_length <= f64::EPSILON {
				return first.color;
			}
			let local = if t >= last.position { t - last.position } else { t + 1. - last.position };
			let adjusted_t = apply_midpoint(local / wrap_length, last.midpoint);
			return interpolate_stop_colors(last.color, first.color, adjusted_t as f32, gradient_space, gradient_hue_direction);
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
				return interpolate_stop_colors(a.color, b.color, adjusted_t as f32, gradient_space, gradient_hue_direction);
			}
		}

		Color::BLACK
	}

	pub fn sort(&mut self, gradient_cyclic: bool) {
		self.sort_returning_new_index(0, gradient_cyclic);
	}

	pub fn reversed(&self, gradient_cyclic: bool) -> Self {
		let count = self.len();
		let mut list = self.reordered((0..count).rev());

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

		// Midpoints belong to the interval to a stop's right, so they shift by one stop as well as flipping;
		// a cyclic gradient's final midpoint is its wrap handle, which flips in place
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

		Self(list)
	}

	pub fn map_colors<F: Fn(&Color) -> Color>(&self, f: F) -> Self {
		let mut mapped = self.clone();
		mapped.0.iter_element_values_mut().for_each(|color| *color = f(color));
		mapped
	}

	/// Build a CSS `linear-gradient(...)` string suitable for use as a `background-image`. Samples the midpoint curves and color space so the rendered gradient matches Graphite's interpolation rather than browser defaults.
	pub fn to_css_linear_gradient(&self, gradient_cyclic: bool, gradient_space: GradientSpace, gradient_hue_direction: GradientHueDirection) -> String {
		if self.len() <= 1 {
			let hex = self.color(0).map(|c| SRGBA8::from(c).to_rgba_hex()).unwrap_or_else(|| "000000ff".to_string());
			return format!("linear-gradient(to right, #{hex} 0%, #{hex} 100%)");
		}
		let pieces = self
			.interpolated_samples(gradient_cyclic, gradient_space, gradient_hue_direction)
			.into_iter()
			.map(|(position, color, _)| {
				let percent = ((position * 100.) * 1e2).round() / 1e2;
				format!("#{} {percent}%", SRGBA8::from(color).to_rgba_hex())
			})
			.collect::<Vec<_>>()
			.join(", ");
		format!("linear-gradient(to right, {pieces})")
	}

	/// Produce a set of linearly-interpolated color samples that approximate the gradient's midpoint curves
	/// and color space.
	///
	/// Each sample is `(position, color, original_midpoint)` where `original_midpoint` is `Some(f64)` with the corresponding
	/// midpoint for actual gradient stops, and `None` for synthesized curve approximation samples.
	///
	/// The downstream SVG/CSS and Vello renderers interpolate between adjacent emitted stops in gamma sRGB space, so the
	/// subdivision emits enough samples that the gamma-drawn segments match the ramp's true curve: the midpoint bias, and
	/// the color space when it is not gamma itself.
	pub fn interpolated_samples(&self, gradient_cyclic: bool, gradient_space: GradientSpace, gradient_hue_direction: GradientHueDirection) -> Vec<(f64, Color, Option<f64>)> {
		/// Controls accuracy vs. number of samples tradeoff.
		/// 2/255 means the linear approximation will deviate by no more than 2 gradations of 8-bit color from the theoretically perfect curve with this midpoint bias.
		const THRESHOLD: f64 = 2. / 255.;

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
			let midpoint_deviates = (y_actual - y_linear).abs() > THRESHOLD;
			let space_deviates = gradient_space != GradientSpace::RgbGamma && {
				let color_left = interpolate_stop_colors(color_a, color_b, y_left as f32, gradient_space, gradient_hue_direction);
				let color_right = interpolate_stop_colors(color_a, color_b, y_right as f32, gradient_space, gradient_hue_direction);
				[0.25, 0.5, 0.75].into_iter().any(|fraction| {
					let y_probe = apply_midpoint(left + (right - left) * fraction, midpoint);
					let color_target = interpolate_stop_colors(color_a, color_b, y_probe as f32, gradient_space, gradient_hue_direction);
					max_gamma_channel_deviation(color_target, color_left.lerp_gamma_srgb(&color_right, fraction as f32)) > THRESHOLD
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

		let stops = self.normalized_stops(gradient_cyclic);
		let count = stops.len();
		if count == 0 {
			return vec![];
		}

		if count == 1 {
			return vec![(stops[0].position, stops[0].color, Some(sanitized_midpoint(stops[0].midpoint)))];
		}

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
		if gradient_cyclic {
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
	#[cfg_attr(feature = "serde", serde(alias = "SrgbLinear"))]
	#[label("Linear (RGB)")]
	RgbLinear,
	/// Interpolates between stops in gamma-encoded RGB, matching classic SVG and CSS gradients.
	#[cfg_attr(feature = "serde", serde(alias = "SrgbGamma"))]
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
		assert_eq!(Gradient::default().evaluate(0.5, Default::default(), false, Default::default(), Default::default()), Color::BLACK);
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

		// The pre-rename field key and variant names from the interim format alias to the current ones
		let renamed_away = json.replace(r#""gradient_space""#, r#""gradient_interpolation""#).replace(r#""OkLab""#, r#""SrgbLinear""#);
		let recovered = serde_json::from_str::<GradientRamp>(&renamed_away).unwrap();
		assert_eq!(recovered.gradient_space, GradientSpace::RgbLinear, "the old key and variant names should decode via their aliases");

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

		let linear = Item::<Gradient>::from(GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE])));
		assert!(
			linear.attribute::<GradientSpace>(ATTR_GRADIENT_SPACE).is_none(),
			"the default Linear must stay absent rather than materialize"
		);
	}

	#[test]
	fn linear_space_densifies_samples_where_gamma_segments_deviate() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		// Gamma needs no synthesized samples since the renderers already draw gamma segments
		assert_eq!(gradient.interpolated_samples(false, GradientSpace::RgbGamma, Default::default()).len(), 2);

		// A linear black-to-white ramp curves away from any single gamma segment, so samples must densify,
		// keeping the end stops in place and every synthesized color on the linear-light line
		let samples = gradient.interpolated_samples(false, GradientSpace::RgbLinear, Default::default());
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
		assert_eq!(flat.interpolated_samples(false, GradientSpace::RgbLinear, Default::default()).len(), 2);
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
					let samples = gradient.interpolated_samples(false, gradient_space, gradient_hue_direction);

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
	fn clear_spread_evaluates_to_transparency_outside_the_unit_range() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		assert_eq!(gradient.evaluate(-0.25, GradientSpread::Clear, false, Default::default(), Default::default()), Color::TRANSPARENT);
		assert_eq!(gradient.evaluate(1.25, GradientSpread::Clear, false, Default::default(), Default::default()), Color::TRANSPARENT);

		for t in [0., 0.25, 1.] {
			assert_eq!(
				gradient.evaluate(t, GradientSpread::Clear, false, Default::default(), Default::default()),
				gradient.evaluate(t, GradientSpread::Pad, false, Default::default(), Default::default()),
				"inside the range Clear must match Pad at t = {t}"
			);
		}
	}

	#[test]
	fn evaluate_follows_the_gradient_space() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		let oklab = gradient.evaluate(0.5, Default::default(), false, GradientSpace::OkLab, Default::default());
		let linear = gradient.evaluate(0.5, Default::default(), false, GradientSpace::RgbLinear, Default::default());
		let gamma = gradient.evaluate(0.5, Default::default(), false, GradientSpace::RgbGamma, Default::default());

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
		let magenta = red_to_blue.evaluate(0.5, Default::default(), false, GradientSpace::Hsl, Default::default());
		for (channel, expected) in [(magenta.r(), 1.), (magenta.g(), 0.), (magenta.b(), 1.)] {
			assert!((channel - expected).abs() < 1e-3, "the HSL mid color of red and blue should be magenta, got {magenta:?}");
		}

		// White's hue is powerless, so an OkLCh interpolation toward it keeps red's hue instead of drifting toward white's arbitrary hue
		let red_to_white = Gradient::from(vec![Color::RED, Color::WHITE]);
		let pink = red_to_white.evaluate(0.5, Default::default(), false, GradientSpace::OkLCh, Default::default());
		let [_, _, red_hue] = color::Oklch::from_linear_srgb([Color::RED.r(), Color::RED.g(), Color::RED.b()]);
		let [_, pink_chroma, pink_hue] = color::Oklch::from_linear_srgb([pink.r(), pink.g(), pink.b()]);
		assert!(pink_chroma > 0.05, "the mid color should stay chromatic, got {pink:?}");
		assert!((pink_hue - red_hue).abs() < 0.5, "the mid hue should hold red's {red_hue} degrees, got {pink_hue}");

		// HSV rides the cube's top face toward white, keeping the mid tint at full brightness where HSL dips
		let tint = red_to_white.evaluate(0.5, Default::default(), false, GradientSpace::Hsv, Default::default());
		for (channel, target) in tint.to_gamma_srgb_channels().into_iter().zip([1., 0.5, 0.5, 1.]) {
			assert!((channel - target).abs() < 1e-3, "the HSV mid tint of red and white should be gamma (1, 0.5, 0.5), got {tint:?}");
		}

		// Toward black both saturation and value halve, the classic HSV shade that neither HSL nor HWB produces
		let red_to_black = Gradient::from(vec![Color::RED, Color::BLACK]);
		let shade = red_to_black.evaluate(0.5, Default::default(), false, GradientSpace::Hsv, Default::default());
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
			let mid = red_to_blue.evaluate(0.5, Default::default(), false, GradientSpace::Hsl, gradient_hue_direction);
			for (channel, target) in [mid.r(), mid.g(), mid.b()].into_iter().zip(expected_rgb) {
				assert!(
					(channel - target).abs() < 1e-3,
					"the {gradient_hue_direction:?} mid of red and blue should be {expected_rgb:?}, got {mid:?}"
				);
			}
		}

		// Identical hues under Longer take a full turn around the wheel, passing through cyan halfway
		let red_to_red = Gradient::from(vec![Color::RED, Color::RED]);
		let mid = red_to_red.evaluate(0.5, Default::default(), false, GradientSpace::Hsl, GradientHueDirection::Longer);
		for (channel, target) in [mid.r(), mid.g(), mid.b()].into_iter().zip([0., 1., 1.]) {
			assert!((channel - target).abs() < 1e-3, "the full-turn mid of red and red should be cyan, got {mid:?}");
		}
	}

	#[test]
	fn gradient_ui_write_back_elides_default_attributes() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		gradient.set_midpoints(&[0.7, 0.5, 0.5]);

		let round_tripped = Gradient::from(&GradientStops::<SRGBA8>::from(&gradient));
		assert!(!round_tripped.has_position_attribute(), "materialized even positions should elide on write-back");
		assert_eq!(round_tripped.midpoints(), vec![0.7, 0.5, 0.5]);
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
		// Stored positions stay as authored, but consumers see them clamped to the 0 to 1 range and sorted
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK, Color::RED]);
		gradient.set_positions(&[1.5, 0.4, -0.5]);
		assert_eq!(gradient.positions(false), vec![1.5, 0.4, -0.5]);

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(false, GradientSpace::RgbGamma, Default::default())
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert!(sample_positions.windows(2).all(|pair| pair[0] <= pair[1]), "samples must ascend: {sample_positions:?}");
		assert_eq!(sample_positions.first(), Some(&0.));
		assert_eq!(sample_positions.last(), Some(&1.));

		assert_eq!(gradient.evaluate(0., Default::default(), false, Default::default(), Default::default()), Color::RED);
		assert_eq!(gradient.evaluate(1., Default::default(), false, Default::default(), Default::default()), Color::WHITE);
	}

	#[test]
	fn infinite_positions_clamp_to_the_range_ends() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[f64::INFINITY, f64::NEG_INFINITY]);

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(false, GradientSpace::RgbGamma, Default::default())
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(gradient.evaluate(0., Default::default(), false, Default::default(), Default::default()), Color::BLACK);
		assert_eq!(gradient.evaluate(1., Default::default(), false, Default::default(), Default::default()), Color::WHITE);
	}

	#[test]
	fn nan_positions_drop_their_stops_from_sampling() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK, Color::RED]);
		gradient.set_positions(&[0., f64::NAN, 1.]);

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(false, GradientSpace::RgbGamma, Default::default())
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(
			gradient.evaluate(0.5, Default::default(), false, GradientSpace::RgbLinear, Default::default()),
			Color::WHITE.lerp(&Color::RED, 0.5)
		);

		// A non-finite position is preserved as nondefault so write-back elision cannot resurrect the dropped stop
		assert!(gradient.nondefault_positions(false).is_some());

		// With every position NaN the gradient samples as stopless, painting solid black to signal the upstream bug
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::RED]);
		gradient.set_positions(&[f64::NAN, f64::NAN]);
		assert!(gradient.interpolated_samples(false, GradientSpace::RgbGamma, Default::default()).is_empty());
		assert_eq!(gradient.evaluate(0.5, Default::default(), false, Default::default(), Default::default()), Color::BLACK);
	}

	#[test]
	fn samples_start_at_the_first_stop_without_synthetic_lead_in() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[0.3, 1.]);

		let samples = gradient.interpolated_samples(false, GradientSpace::RgbGamma, Default::default());
		assert_eq!(samples[0], (0.3, Color::WHITE, None), "renderers that need a flat lead-in before the first stop add it themselves");
	}

	#[test]
	fn nan_midpoints_read_as_linear() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		let linear_result = gradient.evaluate(0.25, Default::default(), false, Default::default(), Default::default());

		gradient.set_midpoints(&[f64::NAN, f64::NAN]);
		assert_eq!(gradient.evaluate(0.25, Default::default(), false, Default::default(), Default::default()), linear_result);
		let no_nan_annotations = gradient
			.interpolated_samples(false, GradientSpace::RgbGamma, Default::default())
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
		let quarter = gradient.evaluate(0.25, Default::default(), true, GradientSpace::RgbLinear, Default::default());
		let wrap_quarter = gradient.evaluate(0.75, Default::default(), true, GradientSpace::RgbLinear, Default::default());
		assert_eq!(quarter, Color::BLACK.lerp(&Color::WHITE, 0.5));
		assert_eq!(wrap_quarter, Color::WHITE.lerp(&Color::BLACK, 0.5));

		// A wrapped interval crossing the 1|0 boundary reads as one continuous span, so its two sides agree at the seam
		let mut offset = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		offset.set_positions(&[0.25, 0.5]);
		let at_end = offset.evaluate(1., Default::default(), true, GradientSpace::RgbLinear, Default::default());
		let at_start = offset.evaluate(0., Default::default(), true, GradientSpace::RgbLinear, Default::default());
		assert_eq!(at_end, at_start, "the 1|0 boundary must be seamless");
		assert_eq!(at_end, Color::WHITE.lerp(&Color::BLACK, 2. / 3.));
	}

	#[test]
	fn cyclic_wrapped_interval_times_with_the_last_stops_midpoint() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		gradient.set_midpoints(&[0.5, 0.25]);

		let expected_t = apply_midpoint(0.5, 0.25);
		let mid = gradient.evaluate(0.75, Default::default(), true, GradientSpace::RgbLinear, Default::default());
		assert_eq!(mid, Color::WHITE.lerp(&Color::BLACK, expected_t as f32));
	}

	#[test]
	fn cyclic_samples_bake_the_wrap_and_play_back_within_tolerance() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::WHITE]);
		gradient.set_positions(&[0.25, 0.5]);
		gradient.set_midpoints(&[0.5, 0.3]);

		let samples = gradient.interpolated_samples(true, GradientSpace::OkLab, Default::default());
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

			let true_color = gradient.evaluate(t, Default::default(), true, GradientSpace::OkLab, Default::default());
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
		let index = gradient.insert_stop(0.75, true, GradientSpace::RgbLinear, Default::default());
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
}
