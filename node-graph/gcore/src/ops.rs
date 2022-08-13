use core::ops::Add;

use crate::Node;

pub struct AddNode;
impl<'n, L: Add<R>, R> Node<'n, (L, R)> for AddNode {
	type Output = <L as Add<R>>::Output;
	fn eval(&'n self, input: (L, R)) -> Self::Output {
		input.0 + input.1
	}
}

pub struct CloneNode;
impl<'n, O: Clone> Node<'n, &'n O> for CloneNode {
	type Output = O;
	fn eval(&'n self, input: &'n O) -> Self::Output {
		input.clone()
	}
}

pub struct FstNode;
impl<'n, T: 'n, U> Node<'n, (T, U)> for FstNode {
	type Output = T;
	fn eval(&'n self, input: (T, U)) -> Self::Output {
		let (a, _) = input;
		a
	}
}

/// Destructures a Tuple of two values and returns the first one
pub struct SndNode;
impl<'n, T, U: 'n> Node<'n, (T, U)> for SndNode {
	type Output = U;
	fn eval(&'n self, input: (T, U)) -> Self::Output {
		let (_, b) = input;
		b
	}
}

/// Return a tuple with two instances of the input argument
pub struct DupNode;
impl<'n, T: Clone> Node<'n, T> for DupNode {
	type Output = (T, T);
	fn eval(&'n self, input: T) -> Self::Output {
		(input.clone(), input) //TODO: use Copy/Clone implementation
	}
}

/// Return the Input Argument
pub struct IdNode;
impl<'n, T> Node<'n, T> for IdNode {
	type Output = T;
	fn eval(&'n self, input: T) -> Self::Output {
		input
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{generic::*, structural::*, value::*};

	#[test]
	pub fn foo() {
		let value = ComposeNode::new(ValueNode(4u32), IdNode);
		let value2 = ValueNode(5u32);
		let dup = DupNode.after(value);
		fn int(_: (), state: &u32) -> &u32 {
			state
		}
		fn swap(input: (u32, u32)) -> (u32, u32) {
			(input.1, input.0)
		}
		let fnn = FnNode::new(&swap);
		let fns = FnNodeWithState::new(int, 42u32);
		assert_eq!(fnn.eval((1u32, 2u32)), (2, 1));
		let _ = fns.eval(());
		let snd = SndNode.after(dup);
		assert_eq!(snd.eval(()), &4u32);
		let sum = AddNode.after(ConsNode(snd)).eval(value2.eval(()));
		assert_eq!(sum, 9);
	}
}
