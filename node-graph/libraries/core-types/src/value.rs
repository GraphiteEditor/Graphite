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
