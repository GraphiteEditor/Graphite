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

impl<T: Clone + Send + Sync + 'static> ValueSource<T> {
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
pub fn record_value_edge<T: Clone + Send + Sync + 'static>(value: T) -> crate::registry::EdgeHandle {
	crate::registry::EdgeHandle::new_record::<T>(std::sync::Arc::new(ValueSource::new(value)) as std::sync::Arc<crate::registry::ErasedRecordNode>)
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
