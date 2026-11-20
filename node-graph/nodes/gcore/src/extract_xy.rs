use core_types::CacheHash;
use dyn_any::DynAny;

// Kept only to deserialize the axis input of old Extract XY nodes for document migration
/// The X or Y component of a vec2.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum XY {
	#[default]
	X,
	Y,
}
