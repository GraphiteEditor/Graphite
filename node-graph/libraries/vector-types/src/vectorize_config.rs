use dyn_any::DynAny;

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum ColorMode {
	#[default]
	Color,
	Binary,
}

impl ColorMode {
	pub fn to_vtracer(&self) -> vtracer::ColorMode {
		match self {
			ColorMode::Color => vtracer::ColorMode::Color,
			ColorMode::Binary => vtracer::ColorMode::Binary,
		}
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum VectorizeMode {
	#[default]
	FullImage,
	PathTrace,
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum Hierarchical {
	#[default]
	Stacked,
	Cutout,
}

impl Hierarchical {
	pub fn to_vtracer(&self) -> vtracer::Hierarchical {
		match self {
			Hierarchical::Stacked => vtracer::Hierarchical::Stacked,
			Hierarchical::Cutout => vtracer::Hierarchical::Cutout,
		}
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum PathSimplifyMode {
	None,
	Polygon,
	#[default]
	Spline,
}

impl PathSimplifyMode {
	pub fn to_vtracer(&self) -> visioncortex::PathSimplifyMode {
		match self {
			PathSimplifyMode::None => visioncortex::PathSimplifyMode::None,
			PathSimplifyMode::Polygon => visioncortex::PathSimplifyMode::Polygon,
			PathSimplifyMode::Spline => visioncortex::PathSimplifyMode::Spline,
		}
	}
}
