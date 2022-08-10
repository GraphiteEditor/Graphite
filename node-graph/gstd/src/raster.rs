use core::marker::PhantomData;
use graphene_core::{value::RefNode, value::ValueNode, Node};

pub struct MapNode<'n, IN: Node<'n, Output = I>, I: Iterator<Item = &'n S>, MAP: Fn(&dyn RefNode<Output = S>) -> MN, MN: Node<'n, Output = O> + 'n, S, O: 'n>(pub IN, pub MAP, PhantomData<&'n (I, S)>);

impl<'n, IN: Node<'n, Output = I>, I: Iterator<Item = &'n S>, MAP: Fn(&dyn RefNode<Output = S>) -> MN, MN: Node<'n, Output = O>, S, O: 'static + Clone> Node<'n> for MapNode<'n, IN, I, MAP, MN, S, O> {
	type Output = Vec<O>;
	fn eval(&'n self) -> Self::Output {
		self.0
			.eval()
			.map(|x| {
				let map_node = self.1(x as &dyn RefNode<Output = S>);
				let result = map_node.eval();
				result.clone()
			})
			.collect()
	}
}
