use std::{borrow::Borrow, marker::PhantomData};

use crate::Node;
pub struct FnNode<T: Fn(&In) -> O, In, O>(T, PhantomData<In>, PhantomData<O>);
impl<T: Fn(&In) -> O, In, O> Node for FnNode<T, In, O> {
    type Output<'a> = O where Self: 'a;
    type Input<'a> = In where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
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
impl<T: Fn(&In, &State) -> O, In, O, State> Node for FnNodeWithState<T, In, O, State> {
    type Output<'a> = O where Self: 'a;
    type Input<'a> = In where Self: 'a;

    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        self.0(input.borrow(), &self.1)
    }
}

impl<T: Fn(&In, &State) -> O, In, O, State> FnNodeWithState<T, In, O, State> {
    pub fn new(f: T, state: State) -> Self {
        FnNodeWithState(f, state, PhantomData::default(), PhantomData::default())
    }
}
