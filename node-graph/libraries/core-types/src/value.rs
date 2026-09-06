/// The node behind every value source: clones its constant onto the record
/// input per evaluation.
pub struct ValueSource<T> {
	value: T,
	layout: crate::record::Layout,
}

impl<T: Clone + Send + Sync + dyn_any::StaticTypeSized> ValueSource<T>
where
	T::Static: Clone + Send + Sync,
{
	pub fn new(value: T) -> Self {
		Self {
			value,
			layout: crate::record::Layout::default().with_writes(0, crate::record::element_write::<T>(), &[]),
		}
	}
}

impl<C, T> crate::node::Node<C> for ValueSource<T>
where
	T: Clone + Send + Sync + dyn_any::StaticTypeSized,
{
	fn serve<'e, 'l>(&self, input: &C, slot: crate::record::FrameClaim<'e, 'l>) -> crate::gpoll::GPoll<crate::record::Served<'e>>
	where
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		slot.lift_served(crate::gpoll::GPoll::Final(self.value.clone()), input.arena())
	}

	fn layout(&self) -> &crate::record::Layout {
		&self.layout
	}
}

/// The native record source of a constant.
pub fn record_value_source<T: Clone + Send + Sync + dyn_any::StaticTypeSized + 'static>(value: T) -> crate::registry::SourceHandle
where
	T::Static: Clone + Send + Sync,
{
	crate::registry::SourceHandle::new_record::<T>(std::sync::Arc::new(ValueSource::new(value)) as std::sync::Arc<crate::registry::ErasedRecordNode>)
}

/// The node behind a leveled value source: a constant list served as one level,
/// one lane per item, with the list's length as the exact extent.
pub struct LeveledValueSource<T> {
	values: Vec<T>,
	layout: crate::record::Layout,
}

impl<T: Clone + Send + Sync + crate::CacheHash + PartialEq + dyn_any::StaticTypeSized> LeveledValueSource<T>
where
	T::Static: Clone + Send + Sync,
{
	pub fn new(values: Vec<T>) -> Self {
		Self {
			values,
			layout: crate::record::Layout::default().with_writes(1, crate::record::element_write_hashed::<T>(), &[]),
		}
	}
}

impl<C, T> crate::node::Node<C> for LeveledValueSource<T>
where
	C: crate::context::ExtractIndex,
	T: Clone + Send + Sync + dyn_any::StaticTypeSized,
{
	fn serve<'e, 'l>(&self, input: &C, slot: crate::record::FrameClaim<'e, 'l>) -> crate::gpoll::GPoll<crate::record::Served<'e>>
	where
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		let Some(value) = self.values.get(input.innermost_index() as usize) else {
			return crate::gpoll::GPoll::error("value level addressed past its items");
		};
		slot.lift_served(crate::gpoll::GPoll::Final(value.clone()), input.arena())
	}

	fn extent_at<'x>(&self, _input: &C, level: u8, _frames: &crate::record::Frames<'x>) -> crate::gpoll::GPoll<crate::gpoll::Extent>
	where
		C: crate::context::ExtractArena<ArenaRef = &'x crate::arena::Arena>,
	{
		match level {
			0 => crate::gpoll::GPoll::Final(crate::gpoll::Extent::Exactly(self.values.len())),
			_ => crate::gpoll::GPoll::Final(crate::gpoll::Extent::Exactly(1)),
		}
	}

	fn layout(&self) -> &crate::record::Layout {
		&self.layout
	}
}

/// The native record source of a constant level: the source type is the element's.
pub fn leveled_record_value_source<T: Clone + Send + Sync + crate::CacheHash + PartialEq + dyn_any::StaticTypeSized + 'static>(values: Vec<T>) -> crate::registry::SourceHandle
where
	T::Static: Clone + Send + Sync,
{
	crate::registry::SourceHandle::new_record::<T>(std::sync::Arc::new(LeveledValueSource::new(values)) as std::sync::Arc<crate::registry::ErasedRecordNode>)
}
