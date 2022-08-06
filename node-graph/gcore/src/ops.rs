use core::{marker::PhantomData, ops::Add};

use crate::{Node, NodeInput};

#[repr(C)]
pub struct AddNode<'n, L: Add<R>, R, I1: Node<'n, Output = L>, I2: Node<'n, Output = R>>(pub I1, pub I2, PhantomData<&'n (L, R)>);
impl<'n, L: Add<R>, R, I1: Node<'n, Output = L>, I2: Node<'n, Output = R>> Node<'n> for AddNode<'n, L, R, I1, I2> {
	type Output = <L as Add<R>>::Output;
	fn eval(&'n self) -> Self::Output {
		self.0.eval() + self.1.eval()
	}
}
impl<'n, L: Add<R>, R, I1: Node<'n, Output = L>, I2: Node<'n, Output = R>> AddNode<'n, L, R, I1, I2> {
	pub fn new(input: (I1, I2)) -> AddNode<'n, L, R, I1, I2> {
		AddNode(input.0, input.1, PhantomData)
	}
}

#[repr(C)]
pub struct CloneNode<'n, N: Node<'n, Output = &'n O>, O: Clone + 'n>(pub N, PhantomData<&'n ()>);
impl<'n, N: Node<'n, Output = &'n O>, O: Clone> Node<'n> for CloneNode<'n, N, O> {
	type Output = O;
	fn eval(&'n self) -> Self::Output {
		self.0.eval().clone()
	}
}
impl<'n, N: Node<'n, Output = &'n O>, O: Clone> CloneNode<'n, N, O> {
	pub const fn new(node: N) -> CloneNode<'n, N, O> {
		CloneNode(node, PhantomData)
	}
}

#[repr(C)]
pub struct FstNode<'n, N: Node<'n>>(pub N, PhantomData<&'n ()>);
impl<'n, T: 'n, U, N: Node<'n, Output = (T, U)>> Node<'n> for FstNode<'n, N> {
	type Output = T;
	fn eval(&'n self) -> Self::Output {
		let (a, _) = self.0.eval();
		a
	}
}

#[repr(C)]
/// Destructures a Tuple of two values and returns the first one
pub struct SndNode<'n, N: Node<'n>>(pub N, PhantomData<&'n ()>);
impl<'n, T, U: 'n, N: Node<'n, Output = (T, U)>> Node<'n> for SndNode<'n, N> {
	type Output = U;
	fn eval(&'n self) -> Self::Output {
		let (_, b) = self.0.eval();
		b
	}
}

#[repr(C)]
/// Return a tuple with two instances of the input argument
pub struct DupNode<'n, N: Node<'n>>(N, PhantomData<&'n ()>);
impl<'n, N: Node<'n>> Node<'n> for DupNode<'n, N> {
	type Output = (N::Output, N::Output);
	fn eval(&'n self) -> Self::Output {
		(self.0.eval(), self.0.eval()) //TODO: use Copy/Clone implementation
	}
}
impl<'n, N: Node<'n>> NodeInput for DupNode<'n, N> {
	type Nodes = N;

	fn new(input: Self::Nodes) -> Self {
		Self(input, PhantomData)
	}
}

#[repr(C)]
/// Return the Input Argument
pub struct IdNode<'n, N: Node<'n>>(N, PhantomData<&'n ()>);
impl<'n, N: Node<'n>> Node<'n> for IdNode<'n, N> {
	type Output = N::Output;
	fn eval(&'n self) -> Self::Output {
		self.0.eval()
	}
}
impl<'n, N: Node<'n>> NodeInput for IdNode<'n, N> {
	type Nodes = N;

	fn new(input: Self::Nodes) -> Self {
		Self(input, PhantomData)
	}
}

pub fn foo() {
	let unit = crate::value::UnitNode;
	let value = IdNode(crate::value::ValueNode(2u32), PhantomData);
	let value2 = crate::value::ValueNode(4u32);
	let dup = DupNode(&value, PhantomData);
	fn int(_: (), state: &u32) -> &u32 {
		state
	}
	fn swap<'n>(input: (&'n u32, &'n u32)) -> (&'n u32, &'n u32) {
		(input.1, input.0)
	}
	let fnn = crate::generic::FnNode::new(swap, &dup);
	let fns = crate::generic::FnNodeWithState::new(int, &unit, 42u32);
	let _ = fnn.eval();
	let _ = fns.eval();
	let snd = SndNode(&fnn, PhantomData);
	let _ = snd.eval();
	let add = AddNode(&snd, value2, PhantomData);
	let _ = add.eval();
}

#[cfg(target_arch = "spirv")]
pub mod gpu {
	//#![deny(warnings)]
	#[repr(C)]
	pub struct PushConsts {
		n: u32,
		node: u32,
	}
	use super::*;
	use crate::{structural::ComposeNodeOwned, Node};
	//use crate::Node;
	use spirv_std::glam::UVec3;
	const ADD: AddNode<u32> = AddNode(PhantomData);
	const OPERATION: ComposeNodeOwned<'_, (u32, u32), u32, FstNode<u32, u32>, DupNode<u32>> = ComposeNodeOwned::new(FstNode(PhantomData, PhantomData), DupNode(PhantomData));

	#[allow(unused)]
	#[spirv(compute(threads(64)))]
	pub fn spread(
		#[spirv(global_invocation_id)] global_id: UVec3,
		#[spirv(storage_buffer, descriptor_set = 0, binding = 0)] a: &[(u32, u32)],
		#[spirv(storage_buffer, descriptor_set = 0, binding = 1)] y: &mut [(u32, u32)],
		#[spirv(push_constant)] push_consts: &PushConsts,
	) {
		fn node_graph(input: Input) -> Output {
			let n0 = ValueNode::new(input);
			let n1 = IdNode::new(n0);
			let n2 = IdNode::new(n1);
			return n2.eval();
		}
		let gid = global_id.x as usize;
		// Only process up to n, which is the length of the buffers.
		if global_id.x < push_consts.n {
			y[gid] = node_graph(a[gid]);
		}
	}
	#[allow(unused)]
	#[spirv(compute(threads(64)))]
	pub fn add(
		#[spirv(global_invocation_id)] global_id: UVec3,
		#[spirv(storage_buffer, descriptor_set = 0, binding = 0)] a: &[(u32, u32)],
		#[spirv(storage_buffer, descriptor_set = 0, binding = 1)] y: &mut [u32],
		#[spirv(push_constant)] push_consts: &PushConsts,
	) {
		let gid = global_id.x as usize;
		// Only process up to n, which is the length of the buffers.
		if global_id.x < push_consts.n {
			y[gid] = ADD.eval(a[gid]);
		}
	}
}
