//! Procedural patterns described by their settings, rendered at whatever resolution is needed of whatever part is seen by the kernels of the `raster-kernels` crate.

use core_types::graphene_hash::CacheHash;
use dyn_any::DynAny;

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

/// The settings of a noise pattern, whose lengths are measured in the document units of the space it's sampled in.
#[derive(Debug, Clone, PartialEq, CacheHash)]
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
