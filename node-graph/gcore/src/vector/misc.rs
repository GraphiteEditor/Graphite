use dyn_any::{DynAny, StaticType};

/// Represents different ways of calculating the centroid.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum CentroidType {
	/// Calculate the Area centroid
	#[default]
	Area,
	/// Calculate the Perimeter centroid
	Perimeter,
}
