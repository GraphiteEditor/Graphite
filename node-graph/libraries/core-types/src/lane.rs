//! The read surface over a source of lanes, so readers name census markers
//! instead of a storage shape.

use crate::attribute::Attribute;

/// One marker's column on a source, resolved once so lane reads skip the key
/// lookup.
pub trait LaneColumn<'a, A: Attribute> {
	/// The lane's value, `None` where the source carries no such column.
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>>;

	/// The lane's value, falling back to the marker's census default.
	fn get(&self, lane: usize) -> A::Value<'a> {
		self.try_get(lane).unwrap_or_else(A::default)
	}
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

	/// Distinguishes an absent column from one holding the census default.
	fn try_attr<A: Attribute>(&self, lane: usize) -> Option<A::Value<'_>> {
		self.column::<A>().try_get(lane)
	}
}

/// A bare element as a one-lane source: no columns, so every marker reads its
/// census default. The read surface for a de-tabled leaf, whose attributes
/// ride the containing lane.
pub struct Single<'a, T>(pub &'a T);

/// The column of a [`Single`]: always absent, so reads fall to the census
/// default.
pub struct NoColumn;

impl<'a, A: Attribute> LaneColumn<'a, A> for NoColumn {
	fn try_get(&self, _lane: usize) -> Option<A::Value<'a>> {
		None
	}
}

impl<T> LaneSource for Single<'_, T> {
	type Element = T;
	type Column<'a, A: Attribute>
		= NoColumn
	where
		Self: 'a;

	fn lane_count(&self) -> usize {
		1
	}

	fn element(&self, lane: usize) -> Option<&T> {
		(lane == 0).then_some(self.0)
	}

	fn column<A: Attribute>(&self) -> NoColumn {
		NoColumn
	}
}

impl<T: crate::bounds::BoundingBox> crate::bounds::BoundingBox for Single<'_, T> {
	fn bounding_box(&self, transform: glam::DAffine2, include_stroke: bool) -> crate::bounds::RenderBoundingBox {
		self.0.bounding_box(transform, include_stroke)
	}

	fn thumbnail_bounding_box(&self, transform: glam::DAffine2, include_stroke: bool) -> crate::bounds::RenderBoundingBox {
		self.0.thumbnail_bounding_box(transform, include_stroke)
	}
}

/// One lane of a source re-based as a one-lane source of a leaf element: the
/// de-tabled leaf read with its containing lane's attributes.
pub struct LeafLane<'a, S, T> {
	source: &'a S,
	index: usize,
	element: &'a T,
}

impl<'a, S, T> LeafLane<'a, S, T> {
	pub fn new(source: &'a S, index: usize, element: &'a T) -> Self {
		Self { source, index, element }
	}
}

pub struct LaneColumnAt<'a, S: LaneSource + 'a, A: Attribute> {
	inner: S::Column<'a, A>,
	index: usize,
}

impl<'a, S: LaneSource, A: Attribute> LaneColumn<'a, A> for LaneColumnAt<'a, S, A> {
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>> {
		(lane == 0).then(|| self.inner.try_get(self.index)).flatten()
	}
}

impl<S: LaneSource, T> LaneSource for LeafLane<'_, S, T> {
	type Element = T;
	type Column<'a, A: Attribute>
		= LaneColumnAt<'a, S, A>
	where
		Self: 'a;

	fn lane_count(&self) -> usize {
		1
	}

	fn element(&self, lane: usize) -> Option<&T> {
		(lane == 0).then_some(self.element)
	}

	fn column<A: Attribute>(&self) -> LaneColumnAt<'_, S, A> {
		LaneColumnAt {
			inner: self.source.column::<A>(),
			index: self.index,
		}
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
