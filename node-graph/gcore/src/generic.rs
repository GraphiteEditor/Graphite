use std::{borrow::Borrow, marker::PhantomData};

use crate::Node;
pub struct FnNode<T: Fn(&In) -> O, In, O>(T, PhantomData<In>, PhantomData<O>);
impl<'n, T: Fn(&In) -> O, In, O: 'n> Node<'n, In> for FnNode<T, In, O> {
    type Output = O;

    fn eval(&'n self, input: &'n In) -> Self::Output {
        self.0(input.borrow())
    }
}

impl<T: Fn(&In) -> O, In, O> FnNode<T, In, O> {
    pub fn new(f: T) -> Self {
        FnNode(f, PhantomData::default(), PhantomData::default())
    }
}

pub struct FnNodeWithState<T: Fn(&In, &State) -> O, In, O, State>(
    T,
    State,
    PhantomData<In>,
    PhantomData<O>,
);
impl<'n, T: Fn(&In, &State) -> O, In, O: 'n, State> Node<'n, In>
    for FnNodeWithState<T, In, O, State>
{
    type Output = O;

    fn eval(&'n self, input: &'n In) -> Self::Output {
        self.0(input.borrow(), &self.1)
    }
}

impl<T: Fn(&In, &State) -> O, In, O, State> FnNodeWithState<T, In, O, State> {
    pub fn new(f: T, state: State) -> Self {
        FnNodeWithState(f, state, PhantomData::default(), PhantomData::default())
    }
}
