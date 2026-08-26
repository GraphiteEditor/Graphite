//! The read surface over a source of lanes, so readers name census markers
//! instead of a storage shape.

use crate::attribute::Attribute;

/// One marker's column on a source, resolved once so lane reads skip the key
/// lookup.
pub trait LaneColumn<'a, A: Attribute> {
	/// The lane's value, or the marker's census default where the column is
	/// absent.
	fn get(&self, lane: usize) -> A::Value<'a>;
}

/// A source of lanes carrying an element and census attributes.
pub trait LaneSource {
	type Element;
	type Column<'a, A: Attribute>: LaneColumn<'a, A>
	where
		Self: 'a;

	fn lane_count(&self) -> usize;

	fn element(&self, lane: usize) -> Option<&Self::Element>;

	fn column<A: Attribute>(&self) -> Self::Column<'_, A>;

	fn attr<A: Attribute>(&self, lane: usize) -> A::Value<'_> {
		self.column::<A>().get(lane)
	}
}

#[cfg(test)]
mod tests {
	use super::LaneSource;
	use crate::attribute::{EditorLayerPath, Opacity, Transform};
	use crate::list::List;
	use crate::uuid::NodeId;
	use glam::DAffine2;

	#[test]
	fn a_plain_marker_reads_what_the_legacy_column_stores() {
		let mut list = List::new_from_element(1u32);
		let transform = DAffine2::from_translation((3., 4.).into());
		list.set_attribute(crate::ATTR_TRANSFORM, 0, transform);

		assert_eq!(list.attr::<Transform>(0), transform);
	}

	#[test]
	fn an_absent_marker_reads_its_census_default_not_the_value_default() {
		let list = List::new_from_element(1u32);

		// `Opacity` declares `= 1.`, so the census default must win over `f64::default()`.
		assert_eq!(list.attr::<Opacity>(0), 1.);
		assert_eq!(list.attr::<Transform>(0), DAffine2::IDENTITY);
	}

	#[test]
	fn a_reference_marker_borrows_the_stored_owned_form() {
		let mut list = List::new_from_element(1u32);
		let path = vec![NodeId(7), NodeId(9)];
		list.set_attribute(crate::ATTR_EDITOR_LAYER_PATH, 0, path.clone());

		assert_eq!(list.attr::<EditorLayerPath>(0), path.as_slice());
		assert!(List::new_from_element(1u32).attr::<EditorLayerPath>(0).is_empty());
	}

	#[test]
	fn a_column_of_the_wrong_stored_type_reads_as_absent() {
		let mut list = List::new_from_element(1u32);
		list.set_attribute(crate::ATTR_OPACITY, 0, "not an f64".to_string());

		assert_eq!(list.attr::<Opacity>(0), 1.);
	}
}
