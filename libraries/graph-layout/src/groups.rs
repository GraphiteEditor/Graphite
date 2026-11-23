use crate::{Graph, NodeIndex};

#[derive(Clone, Debug)]
pub(crate) struct Groups<T> {
	starts: Vec<u32>,
	items: Vec<T>,
}

impl<T: Copy + Default> Groups<T> {
	pub(crate) fn new<I>(graph: &Graph, items: I, group_of: impl Fn(&T) -> NodeIndex) -> Self
	where
		I: DoubleEndedIterator<Item = T> + Clone,
	{
		let mut starts = vec![0_u32; graph.nodes.len() + 1];
		for item in items.clone() {
			starts[group_of(&item).index()] += 1;
		}
		starts.iter_mut().fold(0, |end, start| {
			*start += end;
			*start
		});

		let mut grouped = vec![T::default(); starts.last().copied().unwrap_or_default() as usize];
		for item in items.rev() {
			let start = &mut starts[group_of(&item).index()];
			*start -= 1;
			grouped[*start as usize] = item;
		}

		Self { starts, items: grouped }
	}

	pub(crate) fn of(&self, node: NodeIndex) -> &[T] {
		&self.items[self.starts[node.index()] as usize..self.starts[node.index() + 1] as usize]
	}

	pub(crate) fn sort_each_by_key<K: Ord>(&mut self, key: impl Fn(&T) -> K) {
		self.starts.windows(2).for_each(|range| self.items[range[0] as usize..range[1] as usize].sort_by_key(&key));
	}
}
