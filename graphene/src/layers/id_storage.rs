use std::{
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

// TODO: Simplify this struct to only use a single Vec + HashMap
pub struct UniqueElements<T> {
	/// The IDs of the [Elements] contained within this
	keys: Vec<ElementId>,
	/// The data contained in this
	values: Vec<T>,
	/// Map from element ids to array positions
	id_to_index: HashMap<ElementId, ElementIndex>,

	/// The ID that will be assigned to the next element that is added to this
	#[serde(skip)]
	next_assignment_id: ElementId,
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
	pub fn add(&mut self, element: T, id: Option<ElementId>, insert_index: isize) -> Option<ElementId> {
		let mut insert_index = insert_index as ElementIndex;

		// If the insert index is negative, it is relative to the end
		if insert_index < 0 {
			insert_index = self.values.len() as ElementIndex + insert_index as ElementIndex + 1;
		}

		if insert_index <= self.values.len() as ElementIndex && insert_index >= 0 {
			if let Some(id) = id {
				self.next_assignment_id = id;

				// If the chosen ID is already used, return None
				if self.id_to_index.contains_key(&self.next_assignment_id) {
					return None;
				}
			}

			self.values.insert(insert_index as usize, element);
			self.keys.insert(insert_index as usize, self.next_assignment_id);
			self.id_to_index.insert(self.next_assignment_id, insert_index);

			// Linear probing for collision avoidance
			while self.id_to_index.contains_key(&self.next_assignment_id) {
				self.next_assignment_id += 1;
			}

			Some(self.next_assignment_id)
		} else {
			None
		}
	}

	/// Add a range of elements of type T to this
	/// Insert_index is where the elements should be inserted
	/// Negative values for `insert_index` represent distance from the end
	pub fn add_range<I>(&mut self, elements: I, insert_index: isize) -> Vec<ElementId>
	where
		I: IntoIterator<Item = T>,
	{
		let mut ids = vec![];
		for element in elements {
			if let Some(id) = self.add(element, None, insert_index) {
				ids.push(id);
			}
		}
		ids
	}

	/// Remove an element with a given element ID from the within this container.
	/// This operation will fail if `id` is not present within this container.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::UniqueElements;
	/// let mut folder = PathStorage::default();
	///
	/// // Try to remove a layer that does not exist
	/// assert!(folder.remove_layer(123).is_err());
	///
	/// // Add another folder to the folder
	/// folder.add_layer(PathStorage::default().into(), Some(123), -1);
	///
	/// // Try to remove that folder again
	/// assert!(folder.remove_layer(123).is_ok());
	/// assert_eq!(folder.layers().len(), 0)
	/// ```
	pub fn remove(&mut self, id: ElementId) -> Result<(), DocumentError> {
		let pos = self.position_of_element(id)?;
		self.values.remove(pos);
		self.keys.remove(pos);
		self.id_to_index.remove(&id);
		Ok(())
	}

	/// Returns a list of [ElementId]s in the within this container.
	pub fn keys(&self) -> &[ElementId] {
		self.keys.as_slice()
	}

	/// Get a single element with a given element ID from the within this container.
	pub fn by_id(&self, id: ElementId) -> Option<&T> {
		let pos = self.position_of_element(id).ok()?;
		Some(&self.values[pos])
	}

	/// Get a mutable reference to a single element with a given element ID from the within this container.
	pub fn by_id_mut(&mut self, id: ElementId) -> Option<&mut T> {
		let pos = self.position_of_element(id).ok()?;
		Some(&mut self.values[pos])
	}

	/// Get an element based on its index
	pub fn by_index(&self, index: usize) -> Option<&T> {
		self.values.get(index)
	}

	/// Get a mutable element based on its index
	pub fn by_index_mut(&mut self, index: usize) -> Option<&mut T> {
		self.values.get_mut(index)
	}

	pub fn last(&self) -> Option<&T> {
		self.values.last()
	}

	pub fn last_mut(&mut self) -> Option<&mut T> {
		self.values.last_mut()
	}

	pub fn len(&self) -> usize {
		self.values.len()
	}

	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}

	pub fn clear(&mut self) {
		self.values.clear();
		self.keys.clear();
		self.id_to_index.clear();
	}

	pub fn enumerate(&self) -> impl Iterator<Item = (&ElementId, &T)> {
		self.keys.iter().zip(self.values.iter())
	}

	pub fn enumerate_mut(&mut self) -> impl Iterator<Item = (&ElementId, &mut T)> {
		self.keys.iter().zip(self.values.iter_mut())
	}

	/// Returns `true` if this contains an element with the given [ElementId].
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::UniqueElements;
	/// let mut folder = UniqueElements::default();
	///
	/// // Search for an id that does not exist
	/// assert!(!folder.contains(123));
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(UniqueElements::default().into(), Some(123), -1);
	///
	/// // Search for the id "123"
	/// assert!(folder.contains(123));
	/// ```
	pub fn contains(&self, id: ElementId) -> bool {
		self.id_to_index.contains_key(&id)
	}

	/// Tries to find the index of a layer with the given [ElementId] within the folder.
	/// This operation will fail if no layer with a matching ID is present in the folder.
	///
	/// # Example
	/// ```
	/// # use graphite_graphene::layers::UniqueElements;
	/// let mut folder = UniqueElements::default();
	///
	/// // Search for an id that does not exist
	/// assert!(folder.position_of_element(123).is_err());
	///
	/// // Add layer with the id "123" to the folder
	/// folder.add_layer(UniqueElements::default().into(), Some(123), -1);
	/// folder.add_layer(UniqueElements::default().into(), Some(42), -1);
	///
	/// assert_eq!(folder.position_of_element(123), Ok(0));
	/// assert_eq!(folder.position_of_element(42), Ok(1));
	/// ```
	pub fn position_of_element(&self, element_id: ElementId) -> Result<usize, DocumentError> {
		if let Some(position) = self.id_to_index.get(&element_id) {
			return Ok((*position) as usize);
		}
		Err(DocumentError::LayerNotFound([element_id].into()))
	}
}

impl<T> Default for UniqueElements<T> {
	fn default() -> Self {
		UniqueElements {
			keys: vec![],
			values: vec![],
			next_assignment_id: 0,
			id_to_index: HashMap::new(),
		}
	}
}

impl<T> Deref for UniqueElements<T> {
	type Target = [T];
	fn deref(&self) -> &Self::Target {
		&self.values
	}
}

impl<T> DerefMut for UniqueElements<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.values
	}
}

/// allows use with iterators
/// also allows constructing UniqueElements with collect
impl<A> FromIterator<A> for UniqueElements<A> {
	fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
		let mut new = UniqueElements::default();
		// Add to the end of the existing elements
		new.add_range(iter, -1);
		new
	}
}
