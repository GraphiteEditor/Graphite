use dyn_any::{DynAny, StaticType};
pub use graphene_core::{generic, ops /*, structural*/, Node, RefNode};
use std::marker::PhantomData;

fn fmt_error<I>() -> String {
	format!("DynAnyNode: input is not of correct type, expected {}", std::any::type_name::<I>())
}

pub struct DynAnyNode<'n, N: RefNode<I>, I>(pub N, pub PhantomData<&'n I>);
/*impl<'n, I: StaticType, N: RefNode<'n, &'n I, Output = O> + 'n, O: 'n + StaticType> Node<&'n dyn DynAny<'n>> for DynAnyNode<'n, N, I> {
	type Output = Box<dyn dyn_any::DynAny<'n> + 'n>;
	fn eval(self, input: &'n dyn DynAny<'n>) -> Self::Output {
		let output = self.0.eval_ref(dyn_any::downcast_ref(input).expect(fmt_error::<I>().as_str()));
		Box::new(output)
	}
}*/
/*
impl<'n, I: StaticType, N: RefNode<&'n I, Output = O> + Copy + 'n, O: 'n + StaticType> Node<&'n dyn DynAny<'n>> for &'n DynAnyNode<'n, N, I> {
	type Output = Box<dyn dyn_any::DynAny<'n> + 'n>;
	fn eval(self, input: &'n dyn DynAny<'n>) -> Self::Output {
		let output = self.0.eval_ref(dyn_any::downcast_ref(input).unwrap_or_else(|| panic!("{}", fmt_error::<I>())));
		Box::new(output)
	}
}
impl<'n, I: StaticType, N: RefNode<'n, I, Output = O> + 'n, O: 'n + StaticType> Node<Box<dyn DynAny<'n>>> for DynAnyNode<'n, N, I> {
	type Output = Box<dyn dyn_any::DynAny<'n> + 'n>;
	fn eval(self, input: Box<dyn DynAny<'n>>) -> Self::Output {
		let input: Box<I> = dyn_any::downcast(input).unwrap_or_else(|| panic!("{}", fmt_error::<I>()));
		Box::new(self.0.eval_ref(*input))
	}
}*/
impl<'n, I: StaticType, N: RefNode<I, Output = O> + Copy + 'n, O: 'n + StaticType> Node<Any<'n>> for &'n DynAnyNode<'n, N, I> {
	type Output = Any<'n>;
	fn eval(self, input: Any<'n>) -> Self::Output {
		let input: Box<I> = dyn_any::downcast(input).unwrap_or_else(|| panic!("{}", fmt_error::<I>()));
		Box::new(self.0.eval_ref(*input))
	}
}

impl<'n, I: StaticType, N: RefNode<I, Output = O> + 'n + Copy, O: 'n + StaticType> DynAnyNode<'n, N, I>
where
	N::Output: StaticType,
{
	pub fn new(n: N) -> Self {
		DynAnyNode(n, PhantomData)
	}
	pub fn into_erased(&'n self) -> impl RefNode<Any<'n>, Output = Any<'n>> {
		self
	}
	/*pub fn as_ref(&'n self) -> &'n AnyNode<'n> {
		self
	}
	pub fn into_ref_box(self) -> Box<dyn RefNode<Box<(dyn DynAny<'n> + 'n)>, Output = Box<(dyn DynAny<'n> + 'n)>> + 'n> {
		Box::new(self)
	}*/
	pub fn into_ref(self: &'n &'n Self) -> &'n (dyn RefNode<Any<'n>, Output = Any<'n>> + 'n) {
		self
	}
}

/*impl<'n: 'static, I: StaticType, N, O: 'n + StaticType> DynAnyNode<'n, N, I>
where
	N: RefNode<I, Output = O> + 'n + Copy,
{
	/*pub fn into_owned_erased(self) -> impl RefNode<Any<'n>, Output = Any<'n>> + 'n {
		self
	}*/
	pub fn as_owned(&'n self) -> &'n (dyn RefNode<Any<'n>, Output = Any<'n>> + 'n) {
		self
	}
	/*pub fn into_owned_box(&self) -> Box<dyn DynNodeOwned<'n>> {
		Box::new(self)
	}*/
}*/
pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;
pub type AnyNode<'n> = dyn RefNode<Any<'n>, Output = Any<'n>>;

pub trait DynNodeRef<'n>: RefNode<&'n dyn DynAny<'n>, Output = Box<dyn DynAny<'n> + 'n>> + 'n {}
impl<'n, N: RefNode<&'n dyn DynAny<'n>, Output = Box<dyn DynAny<'n> + 'n>> + 'n> DynNodeRef<'n> for N {}

pub trait DynNodeOwned<'n>: RefNode<Any<'n>, Output = Any<'n>> + 'n {}
impl<'n, N: RefNode<Any<'n>, Output = Any<'n>> + 'n> DynNodeOwned<'n> for N {}

/*impl<'n> Node<Box<dyn DynAny<'n>>> for &'n Box<dyn DynNodeOwned<'n>> {
	type Output = Box<dyn DynAny<'n> + 'n>;
	fn eval(self, input: Box<dyn DynAny<'n>>) -> Self::Output {
		(&*self as &dyn Node<Box<dyn DynAny<'n> + 'n>, Output = Box<dyn DynAny<'n> + 'n>>).eval(input)
	}
}*/

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::ops::{AddNode, IdNode};
	use graphene_core::value::ValueNode;
	/*#[test]
	pub fn dyn_input_compositon() {
		use graphene_core::structural::After;
		use graphene_core::structural::ComposeNode;
		let id: DynAnyNode<_, u32> = DynAnyNode::new(IdNode);
		let add: DynAnyNode<_, (u32, u32)> = DynAnyNode::new(AddNode);
		let value: DynAnyNode<_, ()> = DynAnyNode::new(ValueNode((3u32, 4u32)));
		let id = &id.as_owned();
		let add = add.as_owned();
		let value = value.as_owned();

		/*let computation = ComposeNode::new(value, add);
		let computation = id.after(add.after(value));
		let result: u32 = *dyn_any::downcast(computation.eval(&())).unwrap();*/
	}*/
	#[test]
	#[should_panic]
	pub fn dyn_input_invalid_eval_panic() {
		static ADD: &DynAnyNode<&AddNode, (u32, u32)> = &DynAnyNode(&AddNode, PhantomData);

		let add = ADD.into_ref();
		add.eval_ref(Box::new(&("32", 32u32)));
	}
	/*#[test]
	pub fn dyn_input_storage() {
		let mut vec: Vec<Box<dyn DynNodeRef>> = vec![];
		let id: DynAnyNode<_, u32> = DynAnyNode::new(IdNode);
		let add: DynAnyNode<_, (u32, u32)> = DynAnyNode::new(AddNode);
		let value: DynAnyNode<_, ()> = DynAnyNode::new(ValueNode((3u32, 4u32)));

		vec.push(add.into_ref_box());
		vec.push(id.into_ref_box());
		vec.push(value.into_ref_box());
	}*/
	#[test]
	pub fn dyn_input_storage_composition() {
		let mut vec: Vec<&(dyn RefNode<Any, Output = Any>)> = vec![];
		//let id: DynAnyNode<_, u32> = DynAnyNode::new(IdNode);
		let value: &DynAnyNode<&ValueNode<(u32, u32)>, ()> = &DynAnyNode(&ValueNode((3u32, 4u32)), PhantomData);
		let add: &DynAnyNode<&AddNode, &(u32, u32)> = &DynAnyNode(&AddNode, PhantomData);

		let value_ref = (&value).into_ref();
		let add_ref = (&add).into_ref();
		vec.push(value_ref);
		vec.push(add_ref);
		//vec.push(add.as_owned());
		//vec.push(id.as_owned());
		//let vec = vec.leak();

		let n_value = vec[0];
		let n_add = vec[1];
		//let id = vec[2];

		assert_eq!(*(dyn_any::downcast::<&(u32, u32)>(n_value.eval_ref(Box::new(()))).unwrap()), &(3u32, 4u32));
		fn compose<'n>(
			first: &'n (dyn RefNode<Box<(dyn DynAny<'n> + 'n)>, Output = Box<(dyn DynAny<'n> + 'n)>> + 'n),
			second: &'n (dyn RefNode<Box<(dyn DynAny<'n> + 'n)>, Output = Box<(dyn DynAny<'n> + 'n)>> + 'n),
			input: Any<'n>,
		) -> Any<'n> {
			second.eval_ref(first.eval_ref(input))
		}
		let result = compose(n_value, n_add, Box::new(()));
		assert_eq!(*dyn_any::downcast::<u32>(result).unwrap(), 7u32);
		//let result: u32 = *dyn_any::downcast(computation.eval(Box::new(()))).unwrap();
	}
}
