#[derive(Clone, Copy)]
pub struct ClonedNode<T: Clone>(pub T);

impl<T: Clone, Input> crate::node::Node<Input> for ClonedNode<T> {
	type Output = T;

	fn eval(&self, _input: &Input) -> crate::gpoll::GPoll<T> {
		crate::gpoll::GPoll::Final(self.0.clone())
	}
}

pub fn value_edge<T: Clone + crate::WasmNotSend + crate::WasmNotSync + 'static>(value: T) -> crate::registry::EdgeHandle {
	crate::registry::EdgeHandle::new(std::sync::Arc::new(ClonedNode(value)) as std::sync::Arc<crate::registry::ErasedNode<T>>)
}

/// The node behind every value edge: clones its constant onto the record
/// wire per evaluation.
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

impl<'e, C, T> crate::node::Node<C> for ValueSource<T>
where
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	T: Clone + Send + Sync + 'static,
{
	type Output = crate::record::RecordValue<'e>;

	fn eval(&self, input: &C) -> crate::gpoll::GPoll<crate::record::RecordValue<'e>> {
		crate::record::lift_poll(crate::gpoll::GPoll::Final(self.value.clone()), &self.layout, input.arena())
	}

	fn layout(&self) -> &crate::record::Layout {
		&self.layout
	}
}

/// The native record edge of a constant.
pub fn record_value_edge<T: Clone + Send + Sync + dyn_any::StaticTypeSized + 'static>(value: T) -> crate::registry::EdgeHandle
where
	T::Static: Clone + Send + Sync,
{
	crate::registry::EdgeHandle::new_record::<T>(std::sync::Arc::new(ValueSource::new(value)) as std::sync::Arc<crate::registry::ErasedRecordNode>)
}

/// The node behind a leveled value edge: a constant list served as one level,
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

impl<'e, C, T> crate::node::Node<C> for LeveledValueSource<T>
where
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena> + crate::context::ExtractIndex,
	T: Clone + Send + Sync + 'static,
{
	type Output = crate::record::RecordValue<'e>;

	fn eval(&self, input: &C) -> crate::gpoll::GPoll<crate::record::RecordValue<'e>> {
		let Some(value) = self.values.get(input.innermost_index() as usize) else {
			return crate::gpoll::GPoll::error("value level addressed past its items");
		};
		crate::record::lift_poll(crate::gpoll::GPoll::Final(value.clone()), &self.layout, input.arena())
	}

	fn extent_at(&self, _input: &C, level: u8) -> crate::gpoll::GPoll<crate::gpoll::Extent> {
		match level {
			0 => crate::gpoll::GPoll::Final(crate::gpoll::Extent::Exactly(self.values.len())),
			_ => crate::gpoll::GPoll::Final(crate::gpoll::Extent::Exactly(1)),
		}
	}

	fn layout(&self) -> &crate::record::Layout {
		&self.layout
	}
}

/// The native record edge of a constant level: the edge type is the element's.
pub fn leveled_record_value_edge<T: Clone + Send + Sync + crate::CacheHash + PartialEq + dyn_any::StaticTypeSized + 'static>(values: Vec<T>) -> crate::registry::EdgeHandle
where
	T::Static: Clone + Send + Sync,
{
	crate::registry::EdgeHandle::new_record::<T>(std::sync::Arc::new(LeveledValueSource::new(values)) as std::sync::Arc<crate::registry::ErasedRecordNode>)
}

impl<T: Clone> ClonedNode<T> {
	pub const fn new(value: T) -> ClonedNode<T> {
		ClonedNode(value)
	}
}

impl<T: Clone> From<T> for ClonedNode<T> {
	fn from(value: T) -> Self {
		ClonedNode::new(value)
	}
}
