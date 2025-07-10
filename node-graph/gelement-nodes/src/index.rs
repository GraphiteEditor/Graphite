use glam::DVec2;
use graphene_core::GraphicGroupTable;
use graphene_core::color::Color;
use graphene_core::context::Ctx;
use graphene_core::instances::Instances;
use graphene_core::raster_types::{CPU, RasterDataTable};
use graphene_core::vector::VectorDataTable;

/// Returns the value at the specified index in the collection.
/// If that index has no value, the type's default value is returned.
#[node_macro::node(category("General"))]
fn index<T: AtIndex + Clone + Default>(
	_: impl Ctx,
	/// The collection of data, such as a list or table.
	#[implementations(
		Vec<Color>,
		Vec<Option<Color>>,
		Vec<f64>, Vec<u64>,
		Vec<DVec2>,
		VectorDataTable,
		RasterDataTable<CPU>,
		GraphicGroupTable,
	)]
	collection: T,
	/// The index of the item to retrieve, starting from 0 for the first item.
	index: u32,
) -> T::Output
where
	T::Output: Clone + Default,
{
	collection.at_index(index as usize).unwrap_or_default()
}

pub trait AtIndex {
	type Output;
	fn at_index(&self, index: usize) -> Option<Self::Output>;
}
impl<T: Clone> AtIndex for Vec<T> {
	type Output = T;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		self.get(index).cloned()
	}
}
impl<T: Clone> AtIndex for Instances<T> {
	type Output = Instances<T>;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		let mut result_table = Self::default();
		if let Some(row) = self.instance_ref_iter().nth(index) {
			result_table.push(row.to_instance_cloned());
			Some(result_table)
		} else {
			None
		}
	}
}
