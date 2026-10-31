//! Procedural patterns described by their settings, rendered at whatever resolution is needed of whatever part is seen by the kernels of the `raster-kernels` crate.

use crate::parameters::{ParameterVisitor, ReadParameters, StampParameters};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::Item;
use dyn_any::DynAny;
use glam::DAffine2;
use std::any::Any;

/// Style of noise pattern.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, node_macro::ChoiceType)]
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
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, node_macro::ChoiceType)]
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
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, node_macro::ChoiceType)]
pub enum CellularDistanceFunction {
	#[default]
	Euclidean,
	#[label("Euclidean Squared (Faster)")]
	EuclideanSq,
	Manhattan,
	Hybrid,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, node_macro::ChoiceType)]
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
#[derive(dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, node_macro::ChoiceType)]
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

/// The settings of a noise pattern, whose lengths are measured in the document units of the space it's sampled in.
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

/// A procedural pattern, a function of position in its own space that renders across any area at any resolution.
#[derive(Debug, Clone, PartialEq, CacheHash)]
pub enum GeneratorPattern {
	Noise(NoiseSettings),
	/// The Mandelbrot set, with the unit square spanning -2 to 1 along the real axis and -1.5 to 1.5 along the imaginary axis.
	Mandelbrot,
}

/// A procedural pattern filling the whole plane, which renderers draw at whatever resolution they need across whatever part they see.
/// The item's transform places the pattern's own space, and a noise pattern's settings ride the item as attributes named after the Noise Pattern node's parameters.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny)]
pub enum Generator {
	#[default]
	Noise,
	/// The Mandelbrot set, with the unit square spanning -2 to 1 along the real axis and -1.5 to 1.5 along the imaginary axis.
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
	/// The pattern a generator draws, reading a noise pattern's settings with `attribute`, which looks one of the item's attributes up by key.
	/// Each setting the item lacks takes its default.
	pub fn decode<'a>(generator: Generator, attribute: &'a dyn Fn(&str) -> Option<&'a dyn Any>) -> Self {
		match generator {
			Generator::Noise => {
				let mut settings = NoiseSettings::default();
				settings.visit_parameters(&mut ReadParameters(attribute));
				Self::Noise(settings)
			}
			Generator::Mandelbrot => Self::Mandelbrot,
		}
	}

	/// The pattern a generator item draws.
	pub fn of_item(item: &Item<Generator>) -> Self {
		Self::decode(*item.element(), &|key| item.attributes().get_any(key))
	}
}
