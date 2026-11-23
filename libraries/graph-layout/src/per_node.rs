use crate::{Graph, NodeIndex};
use std::ops::{Index, IndexMut};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PerNode<T>(Vec<T>);

impl<T> PerNode<T> {
	pub(crate) fn from_fn(graph: &Graph, value: impl FnMut(NodeIndex) -> T) -> Self {
		Self(graph.node_indices().map(value).collect())
	}

	pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
		self.0.iter()
	}

	pub(crate) fn indices(&self) -> impl Iterator<Item = NodeIndex> + use<T> {
		(0..u32::try_from(self.0.len()).unwrap_or(u32::MAX)).map(NodeIndex::from)
	}
}

impl<T: Clone> PerNode<T> {
	pub(crate) fn filled(graph: &Graph, value: T) -> Self {
		Self(vec![value; graph.nodes.len()])
	}
}

impl<T> Index<NodeIndex> for PerNode<T> {
	type Output = T;

	fn index(&self, node: NodeIndex) -> &T {
		&self.0[node.index()]
	}
}

impl<T> IndexMut<NodeIndex> for PerNode<T> {
	fn index_mut(&mut self, node: NodeIndex) -> &mut T {
		&mut self.0[node.index()]
	}
}
