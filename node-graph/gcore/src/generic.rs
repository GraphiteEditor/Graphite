use core::marker::PhantomData;

use crate::Node;
pub struct FnNode<'n, T: Fn(<N as Node<'n>>::Output) -> O, N: Node<'n>, O>(
    T,
    N,
    PhantomData<&'n O>,
);
impl<'n, T: Fn(<N as Node<'n>>::Output) -> O, N: Node<'n>, O> Node<'n> for FnNode<'n, T, N, O> {
    type Output = O;

    fn eval(&'n self) -> Self::Output {
        self.0(self.1.eval())
    }
}

impl<'n, T: Fn(<N as Node<'n>>::Output) -> O, N: Node<'n>, O> FnNode<'n, T, N, O> {
    pub fn new(f: T, input: N) -> Self {
        FnNode(f, input, PhantomData)
    }
}

pub struct FnNodeWithState<
    'n,
    T: Fn(<N as Node<'n>>::Output, &'n State) -> O,
    N: Node<'n>,
    O,
    State: 'n,
>(T, N, State, PhantomData<&'n O>);
impl<'n, T: Fn(<N as Node<'n>>::Output, &'n State) -> O, N: Node<'n>, O: 'n, State: 'n> Node<'n>
    for FnNodeWithState<'n, T, N, O, State>
{
    type Output = O;

    fn eval(&'n self) -> Self::Output {
        self.0(self.1.eval(), &self.2)
    }
}

impl<'n, T: Fn(<N as Node<'n>>::Output, &'n State) -> O, N: Node<'n>, O: 'n, State: 'n>
    FnNodeWithState<'n, T, N, O, State>
{
    pub fn new(f: T, input: N, state: State) -> Self {
        FnNodeWithState(f, input, state, PhantomData)
    }
}
