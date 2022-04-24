use core::{marker::PhantomData, ops::Add};

use crate::Node;

#[repr(C)]
#[derive(Default)]
pub struct AddNode<T>(PhantomData<T>);
impl<'n, T: Add + Copy + 'n> Node<'n, (T, T)> for AddNode<T> {
    type Output = <T as Add>::Output;
    fn eval(&'n self, input: (T, T)) -> T::Output {
        let (a, b) = input;
        a + b
    }
}

#[repr(C)]
#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct FstNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<'n, T: Copy + 'n, U> Node<'n, (T, U)> for FstNode<T, U> {
    type Output = T;
    fn eval(&'n self, input: (T, U)) -> Self::Output {
        let (a, _) = input;
        a
    }
}

#[repr(C)]
#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct SndNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<'n, T, U: Copy + 'n> Node<'n, (T, U)> for SndNode<T, U> {
    type Output = U;
    fn eval(&'n self, input: (T, U)) -> Self::Output {
        let (_, b) = input;
        b
    }
}
#[repr(C)]
#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct DupNode<T>(PhantomData<T>);
impl<'n, T: Copy + 'n> Node<'n, T> for DupNode<T> {
    type Output = (T, T);
    fn eval(&'n self, input: T) -> Self::Output {
        (input, input)
    }
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
    const OPERATION: ComposeNodeOwned<'_, (u32, u32), u32, FstNode<u32, u32>, DupNode<u32>> =
        ComposeNodeOwned::new(FstNode(PhantomData, PhantomData), DupNode(PhantomData));

    #[allow(unused)]
    #[spirv(compute(threads(64)))]
    pub fn spread(
        #[spirv(global_invocation_id)] global_id: UVec3,
        #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] a: &[(u32, u32)],
        #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] y: &mut [(u32, u32)],
        #[spirv(push_constant)] push_consts: &PushConsts,
    ) {
        let gid = global_id.x as usize;
        // Only process up to n, which is the length of the buffers.
        if global_id.x < push_consts.n {
            y[gid] = OPERATION.eval(a[gid]);
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
