//! The element-wise map over a graphic's leaves: a modifier defined on one leaf
//! type reaches every leaf of that type in the tree, whatever its depth.

use super::{Graphic, TryFromGraphic};
use core_types::ATTR_TRANSFORM;
use core_types::arena::Arena;
use core_types::attribute::{Attribute, Transform as TransformAttr};
use core_types::graphene_hash::CacheHash;

use core_types::lane::LaneColumn;
use core_types::record::{FieldDesc, FieldWrite, Group, GroupItem, RunBuilder, copy_plan, element_write_hashed};
use dyn_any::Relift;
use glam::DAffine2;
use vector_types::Vector;

/// A leaf a graphic can be mapped over: its `Graphic` variant, plus the record
/// glue a rebuilt run needs to store it.
pub trait MappableLeaf: TryFromGraphic + Clone + Send + Sync + CacheHash + PartialEq + dyn_any::StaticTypeSized + 'static
where
	Self::Static: Clone + Send + Sync,
{
}

impl<T> MappableLeaf for T
where
	T: TryFromGraphic + Clone + Send + Sync + CacheHash + PartialEq + dyn_any::StaticTypeSized + 'static,
	T::Static: Clone + Send + Sync,
{
}

impl<'e> Graphic<'e> {
	/// Applies `map` to every `T` leaf reachable from the graphic, handing each leaf
	/// the transform of the lane holding it and taking back the one it returns.
	/// Groups recurse, since geometry is never inherited, and leaves of other types
	/// pass through untouched. Returns this graphic's own transform, which only a
	/// mapped leaf changes. `None` reports arena exhaustion while rebuilding a group.
	///
	/// A rebuilt group stays resident in `arena`, the form a consumer can read: the
	/// owned form refuses lane reads until something replays it, and an ordinary
	/// element write parks a value rather than running the re-park glue.
	pub fn map<T: MappableLeaf>(&mut self, arena: &'e Arena, transform: DAffine2, map: &mut impl FnMut(T, DAffine2) -> (T, DAffine2)) -> Option<DAffine2>
	where
		T::Static: Clone + Send + Sync,
	{
		if let Some(leaf) = T::leaf_mut(self) {
			let (mapped, transform) = map(leaf.clone(), transform);
			*leaf = mapped;
			return Some(transform);
		}

		match self {
			// The legacy interior is owned outright, so its lanes map in place.
			Graphic::GraphicList(children) => {
				for row in 0..children.len() {
					let lane_transform: DAffine2 = children.attribute_cloned_or_default(ATTR_TRANSFORM, row);
					let Some(child) = children.element_mut(row) else { continue };
					let mapped = child.map::<T>(arena, lane_transform, map)?;
					if mapped != lane_transform {
						children.set_attribute(ATTR_TRANSFORM, row, mapped);
					}
				}
			}
			// A run is shared and cannot be written through, so a mapped lane rebuilds it.
			Graphic::Group(group) => {
				let content = map_run::<T>(&group.content, arena, map)?;
				*self = Graphic::Group(Group { row: group.row.clone(), content });
			}
			_ => {}
		}

		Some(transform)
	}

	/// [`Graphic::map`] over the vector leaves, the shape the vector modifier nodes use.
	pub fn map_vectors(&mut self, arena: &'e Arena, transform: DAffine2, map: &mut impl FnMut(Vector, DAffine2) -> (Vector, DAffine2)) -> Option<DAffine2> {
		self.map::<Vector>(arena, transform, map)
	}
}

/// Content a vector modifier runs over: a bare vector maps directly, a graphic maps
/// every vector leaf it reaches. One kernel then serves both of a modifier's rows.
/// The mapped content lands at the arena's lifetime, since a rebuilt group is resident
/// there, so the result is the content's own type re-stated at that lifetime: exactly what
/// [`Relift`] names. Tying it to the arena is what keeps the mapping safe (the content
/// cannot outlive the frames it now points into), and going through `Relift` rather than a
/// bespoke associated type is what lets the node macro know the mapped element erases to
/// the same static type, so a registry row can name it.
pub trait MapVectorContent: Relift + Sized {
	/// `None` reports arena exhaustion while rebuilding a group.
	fn map_vector_content<'a>(self, arena: &'a Arena, transform: DAffine2, map: &mut impl FnMut(Vector, DAffine2) -> (Vector, DAffine2)) -> Option<(Self::Live<'a>, DAffine2)>;
}

impl MapVectorContent for Vector {
	fn map_vector_content(self, _arena: &Arena, transform: DAffine2, map: &mut impl FnMut(Vector, DAffine2) -> (Vector, DAffine2)) -> Option<(Vector, DAffine2)> {
		Some(map(self, transform))
	}
}

impl MapVectorContent for Graphic<'static> {
	fn map_vector_content<'a>(self, arena: &'a Arena, transform: DAffine2, map: &mut impl FnMut(Vector, DAffine2) -> (Vector, DAffine2)) -> Option<(Graphic<'a>, DAffine2)> {
		// `Graphic` is covariant in its lifetime, so the node's `'static` spelling narrows
		// to the arena's without a cast; the rebuilt run then lands at that same lifetime.
		let mut content: Graphic<'a> = self;
		let transform = content.map_vectors(arena, transform, map)?;

		Some((content, transform))
	}
}

/// The run with every reachable `T` leaf mapped: a run of `T` maps its own lanes,
/// a run of graphics recurses, and any other element type is left alone.
fn map_run<'e, T: MappableLeaf>(item: &GroupItem<'e>, arena: &'e Arena, map: &mut impl FnMut(T, DAffine2) -> (T, DAffine2)) -> Option<GroupItem<'e>>
where
	T::Static: Clone + Send + Sync,
{
	let transforms = core_types::record::RunColumn::<TransformAttr>::of(item);
	let lane_transform = |lane: usize| transforms.try_get(lane).unwrap_or(DAffine2::IDENTITY);

	if let Some(lanes) = item.typed_lanes::<T>() {
		let mapped: Vec<(T, DAffine2)> = (0..lanes.len()).map(|lane| map(lanes.element_ref(lane).clone(), lane_transform(lane))).collect();
		return rebuild_run(item, arena, mapped);
	}

	if let Some(lanes) = item.typed_lanes::<Graphic<'e>>() {
		let mut mapped = Vec::with_capacity(lanes.len());
		for lane in 0..lanes.len() {
			let mut child = lanes.element_ref(lane).clone();
			let transform = child.map::<T>(arena, lane_transform(lane), map)?;
			mapped.push((child, transform));
		}
		return rebuild_run(item, arena, mapped);
	}

	Some(item.clone())
}

/// A fresh run over `mapped`, carrying the source's columns across and writing each
/// lane's returned transform. Only the elements change, so the columns move as bytes
/// rather than through the census: a parked payload carries as its arena reference
/// and stays valid. `None` reports arena exhaustion.
fn rebuild_run<'e, E>(item: &GroupItem<'e>, arena: &'e Arena, mapped: Vec<(E, DAffine2)>) -> Option<GroupItem<'e>>
where
	E: Clone + Send + Sync + CacheHash + PartialEq + dyn_any::StaticTypeSized,
	E::Static: Clone + Send + Sync,
{
	// A lane whose transform the map changed needs the column even where the source run carried none.
	let mut writes: Vec<FieldWrite> = item.layout().fields.iter().map(FieldDesc::as_write).collect();
	if !writes.iter().any(|write| write.name == TransformAttr::NAME && write.level == 0) {
		writes.push(FieldWrite::of::<TransformAttr>(0));
	}

	let mut builder = RunBuilder::new(arena, element_write_hashed::<E>(), &writes, mapped.len())?;
	// The element is written by the push, so the plan carries the columns alone.
	let plan = copy_plan(item.layout(), builder.layout(), false, &[]);

	for (lane, (element, transform)) in mapped.into_iter().enumerate() {
		builder.push(element)?;
		// SAFETY: the plan runs from the source run's own layout into the builder's, and
		// the fresh frames cannot overlap the source.
		unsafe { builder.carry(lane, item.lanes().get(lane).rec(), &plan) };
		builder.attr::<TransformAttr>(lane, transform);
	}

	Some(builder.finish())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::graphic::test_support::unit_square_at;
	use core_types::attribute::Opacity;
	use core_types::lane::LaneSource;
	use core_types::list::{Item, List};
	use core_types::record::{FieldWrite, RunView};
	use glam::DVec2;

	/// A rebuilt run keeps the columns the map never touched, so a modifier
	/// cannot silently drop a lane's blending or layer routing.
	#[test]
	fn a_mapped_run_keeps_its_untouched_columns() {
		let arena = Arena::new(1 << 16).unwrap();
		let mut builder = RunBuilder::new(&arena, element_write_hashed::<Vector>(), &[FieldWrite::of::<TransformAttr>(0), FieldWrite::of::<Opacity>(0)], 2).unwrap();
		for lane in 0..2 {
			builder.push(unit_square_at(DVec2::ZERO)).unwrap();
			builder.attr::<TransformAttr>(lane, DAffine2::from_translation(DVec2::new(lane as f64, 0.)));
		}
		builder.attr::<Opacity>(1, 0.25);
		let content = builder.finish();

		let mut graphic = Graphic::Group(Group { row: None, content });
		let mut seen = Vec::new();
		graphic
			.map_vectors(&arena, DAffine2::IDENTITY, &mut |vector, transform| {
				seen.push(transform);
				(vector, transform * DAffine2::from_scale(DVec2::splat(2.)))
			})
			.expect("the rebuild fits the arena");

		assert_eq!(seen.len(), 2, "every lane of the run is mapped");
		assert_eq!(seen[1], DAffine2::from_translation(DVec2::new(1., 0.)), "each lane is handed its own transform");

		let Graphic::Group(group) = &graphic else { panic!("the group form survives the map") };
		let view = RunView::<Vector>::new(&group.content).expect("the run still holds vectors");
		assert_eq!(
			view.attr::<TransformAttr>(1),
			DAffine2::from_translation(DVec2::new(1., 0.)) * DAffine2::from_scale(DVec2::splat(2.)),
			"the returned transform is written back"
		);
		assert_eq!(view.attr::<Opacity>(1), 0.25, "a column the map never touched survives the rebuild");
	}

	/// A leaf type the map does not target is left alone, so a vector modifier
	/// cannot disturb raster or color content sharing the tree.
	#[test]
	fn a_map_skips_the_leaves_of_other_types() {
		let arena = Arena::new(1 << 16).unwrap();
		let mut children = List::new();
		children.push(Item::new_from_element(Graphic::Color(core_types::Color::WHITE)));
		children.push(Item::new_from_element(Graphic::Vector(unit_square_at(DVec2::ZERO).into())));
		let mut graphic = Graphic::GraphicList(children);

		let mut mapped = 0;
		graphic
			.map_vectors(&arena, DAffine2::IDENTITY, &mut |vector, transform| {
				mapped += 1;
				(vector, transform)
			})
			.expect("no rebuild is needed");

		assert_eq!(mapped, 1, "only the vector leaf is mapped");
		let Graphic::GraphicList(children) = &graphic else { panic!("the list form survives") };
		assert!(matches!(children.element(0), Some(Graphic::Color(_))), "the color leaf passes through untouched");
	}
}
