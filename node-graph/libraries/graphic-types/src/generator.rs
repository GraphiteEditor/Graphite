//! Procedural patterns described by their settings, rendered at whatever resolution is needed of whatever part is seen by the kernels of the `raster-kernels` crate.

use crate::parameters::{ParameterVisitor, ReadParameters, StampParameters};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::Item;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use std::any::Any;

/// Style of noise pattern.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Dropdown)]
pub enum NoiseType {
	#[default]
	Perlin,
	#[label("OpenSimplex2")]
	OpenSimplex2,
	#[label("OpenSimplex2S")]
	OpenSimplex2S,
	Cellular,
	ValueCubic,
	Value,
	WhiteNoise,
}

/// Style of layered levels of the noise pattern.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
pub enum FractalType {
	#[default]
	None,
	#[label("Fractional Brownian Motion")]
	FBm,
	Ridged,
	PingPong,
	#[label("Progressive (Domain Warp Only)")]
	DomainWarpProgressive,
	#[label("Independent (Domain Warp Only)")]
	DomainWarpIndependent,
}

/// Distance function used by the cellular noise.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
pub enum CellularDistanceFunction {
	#[default]
	Euclidean,
	#[label("Euclidean Squared (Faster)")]
	EuclideanSq,
	Manhattan,
	Hybrid,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
pub enum CellularReturnType {
	CellValue,
	#[default]
	#[label("Nearest (F1)")]
	Nearest,
	#[label("Next Nearest (F2)")]
	NextNearest,
	#[label("Average (F1 / 2 + F2 / 2)")]
	Average,
	#[label("Difference (F2 - F1)")]
	Difference,
	#[label("Product (F2 * F1 / 2)")]
	Product,
	#[label("Division (F1 / F2)")]
	Division,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Dropdown)]
pub enum DomainWarpType {
	#[default]
	None,
	#[label("OpenSimplex2")]
	OpenSimplex2,
	#[label("OpenSimplex2 Reduced")]
	OpenSimplex2Reduced,
	BasicGrid,
}

/// The settings of a noise pattern, with lengths measured in the document units of the space it's sampled in.
#[derive(Debug, Clone, Default, PartialEq, CacheHash)]
pub struct NoiseSettings {
	pub seed: i64,
	pub scale: f64,
	pub noise_type: NoiseType,
	pub domain_warp_type: DomainWarpType,
	pub domain_warp_amplitude: f64,
	pub fractal_type: FractalType,
	pub fractal_octaves: i64,
	pub fractal_lacunarity: f64,
	pub fractal_gain: f64,
	pub fractal_weighted_strength: f64,
	pub fractal_ping_pong_strength: f64,
	pub cellular_distance_function: CellularDistanceFunction,
	pub cellular_return_type: CellularReturnType,
	pub cellular_jitter: f64,
}

/// The fractal's family, from the classic Mandelbrot set to variations with their own signature shapes.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
pub enum MandelbrotFractal {
	/// A cardioid body ringed by rounded bulbs and branching filaments (at the default settings).
	#[default]
	ClassicMandelbrot,
	/// A curved triangular body with a bulb-and-spike arm on each corner (at the default settings).
	Tricorn,
	/// An asymmetric body fringed with jagged spires and flame-like detail (at the default settings).
	BurningShip,
}

/// What the brightness outside the set comes from.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Dropdown)]
pub enum MandelbrotExterior {
	/// No brightness throughout, leaving the set as a silhouette.
	Flat,
	#[menu_separator]
	/// How many iterations a point takes to escape, in a flat band for each iteration.
	DiscreteEscape,
	/// How many iterations a point takes to escape, smoothed into soft gradients.
	#[default]
	SmoothEscape,
	#[menu_separator]
	/// How close a point lies to the set's edge, revealing its finest filaments. This is the classic distance estimate, worked out from how quickly the orbit's derivative grows.
	Proximity,
	/// Which way around the set a point lies, as one turn of brightness circling it. This is the set's external angle, worked out from the directions its orbit escapes in, and the counterpart of the internal angle inside each bulb. It levels off when zoomed in far.
	Angle,
	/// Rays radiating out from the set along the directions points escape in. These are the set's external rays, also called its field lines.
	Rays,
	#[menu_separator]
	/// How close a point's orbit comes to the origin before escaping, relative to where its first iteration lands, casting soft shadows around each bulb. This is the classic orbit trap coloring, with the trap at the origin.
	ClosestApproach,
	/// The iteration bringing a point's orbit closest to the origin before it escapes, in a flat band for each, which ring the bulbs like the Closest Iteration's petals inside them.
	ClosestIteration,
	/// Streaks following the paths points take as they escape. This is the classic stripe average coloring, which averages the direction each iteration's value points in.
	Tendrils,
}

/// What the brightness inside the set comes from.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Dropdown)]
pub enum MandelbrotInterior {
	/// Full brightness throughout.
	#[default]
	Flat,
	#[menu_separator]
	/// How many iterations a point's orbit takes to settle into its cycle, in a flat band for each iteration, darkest at each bulb's center. This is the counterpart of the Discrete Escape outside the set.
	DiscreteSettling,
	/// How slowly a point's orbit settles into its cycle, as a smooth gradient darkest at each bulb's center, the counterpart of the Smooth Escape outside the set. It shows how weakly the orbit is drawn together, which is its Lyapunov exponent as the average factor each iteration scales distances by.
	SmoothSettling,
	#[menu_separator]
	/// How far a point lies from its bulb's center toward its edge, measured by how weakly its orbit is drawn into its cycle. This is the bulb's internal radius.
	Radius,
	/// How close a point lies to its bulb's edge, brightening toward the set's edge like the Proximity outside. This is the classic interior distance estimate, worked out from how its cycle's multiplier changes from point to point, so it dims toward the Julia end, where every point inside shares one cycle.
	Proximity,
	/// Which way a point lies from its bulb's center, measured by how its orbit turns around its cycle, giving rays out from each center. This is the bulb's internal angle.
	Angle,
	/// Rays radiating out from each bulb's center along the directions its orbit turns around its cycle, as many for each turn as the Frequency. These are the bulb's internal rays, the counterpart of the external rays outside the set.
	Rays,
	/// How many values a point's orbit cycles through, which is the same across each bulb of the set.
	Period,
	#[menu_separator]
	/// How close a point's orbit comes to the origin, relative to where its first iteration lands.
	ClosestApproach,
	/// The iteration bringing a point's orbit closest to the origin, splitting each bulb into petal-shaped bands. This is the classic closest approach iteration coloring.
	ClosestIteration,
	/// How far from the origin a point's value lies after the last iteration, which is the orbit's final magnitude.
	FinalDistance,
	/// Which way from the origin a point's value lies after the last iteration, which changes along with the number of iterations.
	FinalAngle,
}

/// The settings of a Mandelbrot-style fractal, with Mandelbrot-to-Julia in percent like the Mandelbrot node's parameter.
#[derive(Debug, Clone, Default, PartialEq, CacheHash)]
pub struct MandelbrotSettings {
	pub fractal: MandelbrotFractal,
	pub exterior: MandelbrotExterior,
	pub interior: MandelbrotInterior,
	pub frequency: i64,
	pub scale: f64,
	pub iterations: i64,
	pub power: f64,
	pub mandelbrot_to_julia: f64,
	pub mandelbrot_slice: DVec2,
	pub julia_slice: DVec2,
}

impl MandelbrotSettings {
	/// Stamps every setting onto the generator's item, whatever its value, so which attributes the item carries doesn't depend on them.
	pub fn stamp(&self, item: &mut Item<Generator>) {
		self.clone().visit_parameters(&mut StampParameters(item));
	}

	/// Visits each setting with the attribute it rides, named after the Mandelbrot node's parameter, and its default, which is the node's.
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor) {
		visitor.parameter("fractal", &mut self.fractal, MandelbrotFractal::default());
		visitor.parameter("exterior", &mut self.exterior, MandelbrotExterior::default());
		visitor.parameter("interior", &mut self.interior, MandelbrotInterior::default());
		visitor.parameter("frequency", &mut self.frequency, 1);
		visitor.parameter("scale", &mut self.scale, 100.);
		visitor.parameter("iterations", &mut self.iterations, 256);
		visitor.parameter("power", &mut self.power, 2.);
		visitor.parameter("mandelbrot_to_julia", &mut self.mandelbrot_to_julia, 0.);
		visitor.parameter("mandelbrot_slice", &mut self.mandelbrot_slice, DVec2::ZERO);
		visitor.parameter("julia_slice", &mut self.julia_slice, DVec2::new(-0.8, 0.156));
	}
}

/// A procedural pattern, a function of position in its own space that renders across any area at any resolution.
#[derive(Debug, Clone, PartialEq, CacheHash)]
pub enum GeneratorPattern {
	Noise(NoiseSettings),
	/// A Mandelbrot-style fractal, with its space's origin at the complex plane's and the scale's number of units spanning one unit of the plane.
	Mandelbrot(MandelbrotSettings),
}

/// A procedural pattern filling the whole plane, which renderers draw at whatever resolution they need across whatever part they see.
/// The item's transform places the pattern's own space, and its settings ride the item as attributes named after its node's parameters.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny)]
pub enum Generator {
	#[default]
	Noise,
	/// A Mandelbrot-style fractal, with its space's origin at the complex plane's and the scale's number of units spanning one unit of the plane.
	Mandelbrot,
}

impl std::fmt::Display for Generator {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::Noise => "Noise",
			Self::Mandelbrot => "Mandelbrot",
		})
	}
}

impl BoundingBox for Generator {
	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		match transform.matrix2.determinant() == 0. {
			true => RenderBoundingBox::None,
			false => RenderBoundingBox::Infinite,
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.bounding_box(transform, include_stroke)
	}
}

impl NoiseSettings {
	/// Stamps every setting onto the generator's item, whatever its value, so which attributes the item carries doesn't depend on them.
	pub fn stamp(&self, item: &mut Item<Generator>) {
		self.clone().visit_parameters(&mut StampParameters(item));
	}

	/// Visits each setting with the attribute it rides, named after the Noise Pattern node's parameter, and its default, which is the node's.
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor) {
		visitor.parameter("seed", &mut self.seed, 0);
		visitor.parameter("scale", &mut self.scale, 10.);
		visitor.parameter("noise_type", &mut self.noise_type, NoiseType::default());
		visitor.parameter("domain_warp_type", &mut self.domain_warp_type, DomainWarpType::default());
		visitor.parameter("domain_warp_amplitude", &mut self.domain_warp_amplitude, 100.);
		visitor.parameter("fractal_type", &mut self.fractal_type, FractalType::default());
		visitor.parameter("fractal_octaves", &mut self.fractal_octaves, 3);
		visitor.parameter("fractal_lacunarity", &mut self.fractal_lacunarity, 2.);
		visitor.parameter("fractal_gain", &mut self.fractal_gain, 0.5);
		visitor.parameter("fractal_weighted_strength", &mut self.fractal_weighted_strength, 0.);
		visitor.parameter("fractal_ping_pong_strength", &mut self.fractal_ping_pong_strength, 2.);
		visitor.parameter("cellular_distance_function", &mut self.cellular_distance_function, CellularDistanceFunction::default());
		visitor.parameter("cellular_return_type", &mut self.cellular_return_type, CellularReturnType::default());
		visitor.parameter("cellular_jitter", &mut self.cellular_jitter, 1.);
	}
}

impl GeneratorPattern {
	/// The pattern a generator draws, reading its settings with `attribute`, which looks one of the item's attributes up by key.
	/// Each setting the item lacks takes its default.
	pub fn decode<'a>(generator: Generator, attribute: &'a dyn Fn(&str) -> Option<&'a dyn Any>) -> Self {
		match generator {
			Generator::Noise => {
				let mut settings = NoiseSettings::default();
				settings.visit_parameters(&mut ReadParameters(attribute));
				Self::Noise(settings)
			}
			Generator::Mandelbrot => {
				let mut settings = MandelbrotSettings::default();
				settings.visit_parameters(&mut ReadParameters(attribute));
				Self::Mandelbrot(settings)
			}
		}
	}

	/// The pattern a generator item draws.
	pub fn of_item(item: &Item<Generator>) -> Self {
		Self::decode(*item.element(), &|key| item.attributes().get_any(key))
	}
}
