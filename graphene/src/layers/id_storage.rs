use std::{
	cell::RefCell,
	collections::HashMap,
	ops::{Deref, DerefMut},
};

use crate::DocumentError;
use serde::{Deserialize, Serialize};

type ElementId = u64;
type ElementIndex = i64;

/// A layer that encapsulates other layers, including potentially more folders.
/// The contained layers are rendered in the same order they are
/// stored in the [layers](PathStorage::layers) field.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]

// TODO: Default is a bit weird because Layer does not implement Default. but we should not care because the empty vec is the default
pub struct UniqueElements<T> {
	/// The ID that will be assigned to the next element that is added to this
	next_assignment_id: ElementId,
	/// Map from element ids to array positions
	id_map: HashMap<ElementId, RefCell<T>>,
}

impl<T> UniqueElements<T> {
	/// When a insertion ID is provided, try to insert the element with the given ID.
	/// If that ID is already used, return `None`.
	/// When no insertion ID is provided, search for the next free ID and insert it with that.
	/// Negative values for `insert_index` represent distance from the end
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::shape_layer::ShapeLayer;
	/// # use graphite_graphene::layers::UniqueElements;
	/// # use graphite_graphene::layers::style::PathStyle;
	/// # use graphite_graphene::layers::layer_info::LayerDataType;
	/// let mut folder = UniqueElements::default();
	///
	/// // Create two layers to be added to the folder
	/// let mut shape_layer = ShapeLayer::rectangle(PathStyle::default());
	/// let mut folder_layer = UniqueElements::default();
	///
	/// folder.add(shape_layer.into(), None, -1);
	/// folder.add(folder_layer.into(), Some(123), 0);
	/// ```
	pub fn insert(&mut self, element: T, id: Option<ElementId>) -> Option<ElementId> {
		if let Some(new_id) = id {
			self.next_assignment_id = new_id
		}
		match self.id_map.insert(self.next_assignment_id, RefCell::new(element)) {
			None => None,
			Some(_) => Some(self.next_assignment_id),
		}
	}
}

impl<T> Default for UniqueElements<T> {
	fn default() -> Self {
		UniqueElements {
			next_assignment_id: 0,
			id_map: HashMap::new(),
		}
	}
}

impl<T> Deref for UniqueElements<T> {
	type Target = HashMap<ElementId, RefCell<T>>;
	fn deref(&self) -> &Self::Target {
		&self.id_map
	}
}

impl<T> DerefMut for UniqueElements<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.id_map
	}
}

/// allows use with iterators
/// also allows constructing UniqueElements with collect
impl<A> FromIterator<A> for UniqueElements<A> {
	fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
		let mut new = UniqueElements::default();
		iter.into_iter().for_each(|element| match new.insert(element, None) {
			_ => (), // attempt to add all elements, even if one fails
		});
		new
	}
}

impl<'b, 'a: 'b, A: 'a + Clone> FromIterator<&'b A> for UniqueElements<A> {
	fn from_iter<T: IntoIterator<Item = &'b A>>(iter: T) -> Self {
		let mut new = UniqueElements::default();
		iter.into_iter().for_each(|element| match new.insert(element.clone(), None) {
			_ => (), // attempt to add all elements, even if one fails
		});
		new
	}
}
