use super::path::{Path, curve_type_at};
use core_types::list::List;

/// One path's reference to a pool point, naming where in the weave that point is used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reference {
	pub path: u32,
	pub index: u32,
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
			for &point in path.0.iter_element_values() {
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
			for (reference_index, &point) in path.0.iter_element_values().enumerate() {
				let Some(cursor) = cursors.get_mut(point as usize) else { continue };
				references[*cursor as usize] = Reference {
					path: path_index as u32,
					index: reference_index as u32,
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

	/// The segment ends meeting at this point: 1 at a free end, 2 along a chain, 3 or more at a junction.
	/// Handle references contribute none, since only anchors have segments radiating from them.
	pub fn spokes(&self, point: u32, paths: &List<Path>) -> usize {
		self.references(point)
			.iter()
			.filter_map(|reference| {
				let path = paths.element(reference.path as usize)?;
				if !path.is_anchor_reference(reference.index as usize, curve_type_at(paths, reference.path as usize)) {
					return None;
				}

				let last = path.0.len().saturating_sub(1) as u32;
				Some(usize::from(reference.index > 0) + usize::from(reference.index < last))
			})
			.sum()
	}
}

#[cfg(test)]
mod tests {
	use super::super::path::CurveType;
	use super::*;
	use core_types::list::{ATTR_CURVE_TYPE, Item};

	fn paths(specs: &[(CurveType, &[u32])]) -> List<Path> {
		let mut paths = List::new();

		for &(curve_type, references) in specs {
			let path = Path(references.iter().map(|&reference| Item::new_from_element(reference)).collect());
			paths.push(Item::new_from_element(path).with_attribute(ATTR_CURVE_TYPE, curve_type));
		}

		paths
	}

	// A closed circle crossed by a chord welded at two of its anchors, plus two points no path names
	#[test]
	fn spokes_at_welds_and_free_points() {
		let paths = paths(&[(CurveType::Cubic, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0]), (CurveType::Linear, &[0, 12, 6])]);
		let connectivity = Connectivity::build(15, &paths);

		// The seam contributes two spokes and the chord a third
		assert_eq!(connectivity.spokes(0, &paths), 3);
		assert_eq!(connectivity.spokes(6, &paths), 3);

		assert_eq!(connectivity.spokes(3, &paths), 2);
		assert_eq!(connectivity.spokes(12, &paths), 2);

		// Handles carry no segments, and unreferenced points are free
		assert_eq!(connectivity.spokes(1, &paths), 0);
		assert!(!connectivity.is_referenced(13));
		assert!(connectivity.is_referenced(12));
	}

	#[test]
	fn references_name_every_use() {
		let paths = paths(&[(CurveType::Linear, &[0, 1, 2]), (CurveType::Linear, &[2, 3])]);
		let connectivity = Connectivity::build(4, &paths);

		assert_eq!(connectivity.references(0), [Reference { path: 0, index: 0 }]);
		assert_eq!(connectivity.references(2), [Reference { path: 0, index: 2 }, Reference { path: 1, index: 0 }]);
		assert_eq!(connectivity.spokes(2, &paths), 2);
	}

	// References falling anywhere outside the pool are ignored, including one exactly at its end
	#[test]
	fn dangling_references_are_ignored() {
		let paths = paths(&[(CurveType::Linear, &[0, 3, 1, 4])]);
		let connectivity = Connectivity::build(3, &paths);

		assert_eq!(connectivity.references(0), [Reference { path: 0, index: 0 }]);
		assert_eq!(connectivity.references(1), [Reference { path: 0, index: 2 }]);
		assert_eq!(connectivity.references(3), []);
		assert_eq!(connectivity.references(4), []);
	}

	// An open end has one spoke, which is what makes it a cap site
	#[test]
	fn free_ends_have_one_spoke() {
		let paths = paths(&[(CurveType::Linear, &[0, 1, 2])]);
		let connectivity = Connectivity::build(3, &paths);

		assert_eq!(connectivity.spokes(0, &paths), 1);
		assert_eq!(connectivity.spokes(2, &paths), 1);
		assert_eq!(connectivity.spokes(1, &paths), 2);
	}
}
