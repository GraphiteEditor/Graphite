use super::path::{CurveType, Path, curve_type_at};
use core_types::list::List;

/// One path's reference to a pool point, naming where in the weave that point is used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reference {
	pub path: u32,
	pub index: u32,
	/// The curve type of the path holding this reference. Its stride decides whether the reference names an anchor.
	pub curve_type: CurveType,
	/// The segment ends radiating from this reference: none for a handle or a path's only reference, one at a path end, two along a path.
	pub spokes: u8,
}

impl Reference {
	/// Whether this reference sits at an anchor slot of its path rather than a handle slot.
	pub fn is_anchor(&self) -> bool {
		(self.index as usize).is_multiple_of(self.curve_type.stride())
	}
}

/// Reverse index from each pool point to the references naming it.
///
/// Paths store their own traversal order, so connectivity never has to be searched for. This index
/// answers the remaining question of which paths meet at a point, built in one pass over every
/// reference and read afterwards as a slice per point.
#[derive(Clone, Debug, Default)]
pub struct Connectivity {
	starts: Vec<u32>,
	references: Vec<Reference>,
}

impl Connectivity {
	/// Indexes every reference by the pool point it names, in O(points + references).
	pub fn build(point_count: usize, paths: &List<Path>) -> Self {
		let mut starts = vec![0_u32; point_count + 1];
		for path in paths.iter_element_values() {
			for point in path.references() {
				if let Some(count) = (point as usize).checked_add(1).and_then(|slot| starts.get_mut(slot)) {
					*count += 1;
				}
			}
		}

		// Summing the per-point counts in place turns them into the offset where each point's run begins
		for index in 1..starts.len() {
			starts[index] += starts[index - 1];
		}

		let mut cursors = starts[..point_count].to_vec();
		let mut references = vec![Reference::default(); starts[point_count] as usize];
		for (path_index, path) in paths.iter_element_values().enumerate() {
			// Each path's curve type is read once here, so queries never look it up again
			let curve_type = curve_type_at(paths, path_index);
			let last = path.0.len().saturating_sub(1);

			for (reference_index, point) in path.references().enumerate() {
				let Some(cursor) = cursors.get_mut(point as usize) else { continue };

				let anchor = reference_index.is_multiple_of(curve_type.stride());
				let spokes = if anchor { u8::from(reference_index > 0) + u8::from(reference_index < last) } else { 0 };

				references[*cursor as usize] = Reference {
					path: path_index as u32,
					index: reference_index as u32,
					curve_type,
					spokes,
				};
				*cursor += 1;
			}
		}

		Self { starts, references }
	}

	/// The references naming the given pool point, ordered by path.
	pub fn references(&self, point: u32) -> &[Reference] {
		let (Some(&start), Some(&end)) = (self.starts.get(point as usize), (point as usize).checked_add(1).and_then(|next| self.starts.get(next))) else {
			return &[];
		};

		&self.references[start as usize..end as usize]
	}

	/// Whether any path names this point, false for a free point carried by the pool alone.
	pub fn is_referenced(&self, point: u32) -> bool {
		!self.references(point).is_empty()
	}

	/// Whether this point is a handle, named by paths only at handle slots.
	pub fn is_handle(&self, point: u32) -> bool {
		let references = self.references(point);
		!references.is_empty() && !references.iter().any(Reference::is_anchor)
	}

	/// The segment ends meeting at this point: 1 at a free end, 2 along a chain, 3 or more at a junction.
	/// Handle references contribute none, since only anchors have segments radiating from them.
	pub fn spokes(&self, point: u32) -> usize {
		self.references(point).iter().map(|reference| usize::from(reference.spokes)).sum()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::list::{ATTR_CURVE_TYPE, Item};

	fn paths(specs: &[(CurveType, &[u32])]) -> List<Path> {
		let mut paths = List::new();

		for &(curve_type, references) in specs {
			let path = Path::from_references(references.iter().copied());
			paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, curve_type));
		}

		paths
	}

	fn places(connectivity: &Connectivity, point: u32) -> Vec<(u32, u32)> {
		connectivity.references(point).iter().map(|reference| (reference.path, reference.index)).collect()
	}

	// A closed circle crossed by a chord welded at two of its anchors, plus two points no path names
	#[test]
	fn spokes_at_welds_and_free_points() {
		let paths = paths(&[(CurveType::Cubic, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0]), (CurveType::Linear, &[0, 12, 6])]);
		let connectivity = Connectivity::build(15, &paths);

		// The seam contributes two spokes and the chord a third
		assert_eq!(connectivity.spokes(0), 3);
		assert_eq!(connectivity.spokes(6), 3);

		assert_eq!(connectivity.spokes(3), 2);
		assert_eq!(connectivity.spokes(12), 2);

		// Handles carry no segments, and unreferenced points are free
		assert_eq!(connectivity.spokes(1), 0);
		assert!(connectivity.is_handle(1));
		assert!(!connectivity.is_handle(0));
		assert!(!connectivity.is_handle(13));
		assert!(!connectivity.is_referenced(13));
		assert!(connectivity.is_referenced(12));
	}

	#[test]
	fn references_name_every_use() {
		let paths = paths(&[(CurveType::Linear, &[0, 1, 2]), (CurveType::Linear, &[2, 3])]);
		let connectivity = Connectivity::build(4, &paths);

		assert_eq!(places(&connectivity, 0), [(0, 0)]);
		assert_eq!(places(&connectivity, 2), [(0, 2), (1, 0)]);
		assert_eq!(connectivity.spokes(2), 2);
	}

	// References falling anywhere outside the pool are ignored, including one exactly at its end
	#[test]
	fn dangling_references_are_ignored() {
		let paths = paths(&[(CurveType::Linear, &[0, 3, 1, 4])]);
		let connectivity = Connectivity::build(3, &paths);

		assert_eq!(places(&connectivity, 0), [(0, 0)]);
		assert_eq!(places(&connectivity, 1), [(0, 2)]);
		assert!(places(&connectivity, 3).is_empty());
		assert!(places(&connectivity, 4).is_empty());
	}

	// An open end has one spoke, which is what makes it a cap site
	#[test]
	fn free_ends_have_one_spoke() {
		let paths = paths(&[(CurveType::Linear, &[0, 1, 2])]);
		let connectivity = Connectivity::build(3, &paths);

		assert_eq!(connectivity.spokes(0), 1);
		assert_eq!(connectivity.spokes(2), 1);
		assert_eq!(connectivity.spokes(1), 2);
	}
}
