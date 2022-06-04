use core::marker::PhantomData;

use crate::Node;
pub struct FnNode<'n, T: Fn(<N as Node>::Output) -> O, N: Node<'n>, O: 'n>(
    T,
    &'n N,
    PhantomData<O>,
);
impl<'n, T: Fn(<N as Node>::Output) -> O, N: Node<'n>, O> Node<'n> for FnNode<'n, T, N, O> {
    type Output = O;

    fn eval(&'n self) -> Self::Output {
        self.0(self.1.eval())
    }
}

impl<'n, T: Fn(<N as Node>::Output) -> O, N: Node<'n>, O> FnNode<'n, T, N, O> {
    pub fn new(f: T, input: &'n N) -> Self {
        FnNode(f, input, PhantomData)
    }
}

pub struct FnNodeWithState<'n, T: Fn(N::Output, &State) -> O, N: Node<'n>, O, State>(
    T,
    &'n N,
    State,
    PhantomData<O>,
);
impl<'n, T: Fn(N::Output, &State) -> O, N: Node<'n>, O: 'n, State> Node<'n>
    for FnNodeWithState<'n, T, N, O, State>
{
    type Output = O;

    fn eval(&'n self) -> Self::Output {
        self.0(self.1.eval(), &self.2)
    }
}

impl<'n, T: Fn(N::Output, &State) -> O, N: Node<'n>, O, State> FnNodeWithState<'n, T, N, O, State> {
    pub fn new(f: T, input: &'n N, state: State) -> Self {
        FnNodeWithState(f, input, state, PhantomData)
    }
}
