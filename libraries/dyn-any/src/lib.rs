#![doc(html_root_url = "http://docs.rs/const-default/1.0.0")]
#![cfg_attr(feature = "unstable-docs", feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::missing_safety_doc)]
#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "derive")]
#[cfg_attr(feature = "unstable-docs", doc(cfg(feature = "derive")))]
pub use dyn_any_derive::DynAny;

/// Implement this trait for your `dyn Trait` types for all `T: Trait`
pub trait UpcastFrom<T: ?Sized> {
	fn up_from(value: &T) -> &Self;
	fn up_from_mut(value: &mut T) -> &mut Self;
	#[cfg(feature = "alloc")]
	fn up_from_box(value: Box<T>) -> Box<Self>;
}

/// Use this trait to perform your upcasts on dyn traits. Make sure to require it in the supertrait!
pub trait Upcast<U: ?Sized> {
	fn up(&self) -> &U;
	fn up_mut(&mut self) -> &mut U;
	#[cfg(feature = "alloc")]
	fn up_box(self: Box<Self>) -> Box<U>;
}

impl<T: ?Sized, U: ?Sized> Upcast<U> for T
where
	U: UpcastFrom<T>,
{
	fn up(&self) -> &U {
		U::up_from(self)
	}
	fn up_mut(&mut self) -> &mut U {
		U::up_from_mut(self)
	}
	#[cfg(feature = "alloc")]
	fn up_box(self: Box<Self>) -> Box<U> {
		U::up_from_box(self)
	}
}

use core::any::TypeId;

impl<'a, T: DynAny<'a> + 'a> UpcastFrom<T> for dyn DynAny<'a> + 'a {
	fn up_from(value: &T) -> &(dyn DynAny<'a> + 'a) {
		value
	}
	fn up_from_mut(value: &mut T) -> &mut (dyn DynAny<'a> + 'a) {
		value
	}
	#[cfg(feature = "alloc")]
	fn up_from_box(value: Box<T>) -> Box<Self> {
		value
	}
}

pub trait DynAny<'a>: 'a {
	fn type_id(&self) -> TypeId;
	#[cfg(feature = "log-bad-types")]
	fn type_name(&self) -> &'static str;
}

impl<'a, T: StaticType + 'a> DynAny<'a> for T {
	fn type_id(&self) -> core::any::TypeId {
		core::any::TypeId::of::<T::Static>()
	}
	#[cfg(feature = "log-bad-types")]
	fn type_name(&self) -> &'static str {
		core::any::type_name::<T>()
	}
}
pub fn downcast_ref<'a, V: StaticType + 'a>(i: &'a dyn DynAny<'a>) -> Option<&'a V> {
	if i.type_id() == core::any::TypeId::of::<<V as StaticType>::Static>() {
		// SAFETY: caller guarantees that T is the correct type
		let ptr = i as *const dyn DynAny<'a> as *const V;
		Some(unsafe { &*ptr })
	} else {
		None
	}
}

#[cfg(feature = "alloc")]
pub fn downcast<'a, V: StaticType + 'a>(i: Box<dyn DynAny<'a> + 'a>) -> Result<Box<V>, String> {
	let type_id = DynAny::type_id(i.as_ref());
	if type_id == core::any::TypeId::of::<<V as StaticType>::Static>() {
		// SAFETY: caller guarantees that T is the correct type
		let ptr = Box::into_raw(i) as *mut V;
		Ok(unsafe { Box::from_raw(ptr) })
	} else {
		if type_id == core::any::TypeId::of::<&dyn DynAny<'static>>() {
			panic!("downcast error: type_id == core::any::TypeId::of::<dyn DynAny<'a>>()");
		}
		#[cfg(feature = "log-bad-types")]
		{
			Err(format!("Incorrect type, expected {} but found {}", core::any::type_name::<V>(), DynAny::type_name(i.as_ref())))
		}

		#[cfg(not(feature = "log-bad-types"))]
		{
			Err(format!("Incorrect type, expected {}", core::any::type_name::<V>()))
		}
	}
}

pub unsafe trait StaticType {
	type Static: 'static + ?Sized;
	fn type_id(&self) -> core::any::TypeId {
		core::any::TypeId::of::<Self::Static>()
	}
}

pub unsafe trait StaticTypeSized {
	type Static: 'static;
	fn type_id(&self) -> core::any::TypeId {
		core::any::TypeId::of::<<Self as StaticTypeSized>::Static>()
	}
}
unsafe impl<T: StaticType + Sized> StaticTypeSized for T
where
	T::Static: Sized,
{
	type Static = <T as StaticType>::Static;
}
pub unsafe trait StaticTypeClone {
	type Static: 'static + Clone;
	fn type_id(&self) -> core::any::TypeId {
		core::any::TypeId::of::<<Self as StaticTypeClone>::Static>()
	}
}
unsafe impl<T: StaticType + Clone> StaticTypeClone for T
where
	T::Static: Clone,
{
	type Static = <T as StaticType>::Static;
}

macro_rules! impl_type {
	($($id:ident$(<$($(($l:lifetime, $s:lifetime)),*|)?$($T:ident),*>)?),*) => {
		$(
		unsafe impl< $($($T:  $crate::StaticTypeSized ,)*)?> $crate::StaticType for $id $(<$($($l,)*)?$($T, )*>)?{
			type Static = $id$(<$($($s,)*)?$(<$T as $crate::StaticTypeSized>::Static,)*>)?;
		}
		)*
	};
}

#[cfg(feature = "alloc")]
unsafe impl<'a, T: StaticTypeClone + Clone> StaticType for Cow<'a, T> {
	type Static = Cow<'static, <T as StaticTypeClone>::Static>;
}
unsafe impl<T: StaticTypeSized> StaticType for *const [T] {
	type Static = *const [<T as StaticTypeSized>::Static];
}
unsafe impl<T: StaticTypeSized> StaticType for *mut [T] {
	type Static = *mut [<T as StaticTypeSized>::Static];
}
macro_rules! impl_slice {
	($($id:ident),*) => {
		$(
		unsafe impl<'a, T: StaticTypeSized> StaticType for $id<'a, T> {
			type Static = $id<'static, <T as StaticTypeSized>::Static>;
		}
		)*
	};
}

mod slice {
	use super::*;
	use core::slice::*;
	impl_slice!(Iter, IterMut, Chunks, ChunksMut, RChunks, RChunksMut, Windows);
}

#[cfg(feature = "alloc")]
unsafe impl<'a, T: StaticTypeSized> StaticType for Box<dyn Iterator<Item = T> + 'a + Send + Sync> {
	type Static = Box<dyn Iterator<Item = <T as StaticTypeSized>::Static> + Send + Sync>;
}

unsafe impl<'a> StaticType for &'a str {
	type Static = &'static str;
}
unsafe impl StaticType for () {
	type Static = ();
}
unsafe impl<'a, T: 'a + StaticType + ?Sized> StaticType for &'a T {
	type Static = &'static <T as StaticType>::Static;
}
unsafe impl<T: StaticTypeSized, const N: usize> StaticType for [T; N] {
	type Static = [<T as StaticTypeSized>::Static; N];
}
unsafe impl<T: StaticTypeSized> StaticType for [T] {
	type Static = [<T as StaticTypeSized>::Static];
}

unsafe impl StaticType for dyn for<'i> DynAny<'_> + '_ {
	type Static = dyn DynAny<'static>;
}
unsafe impl StaticType for dyn for<'i> DynAny<'_> + Send + Sync + '_ {
	type Static = dyn DynAny<'static> + Send + Sync;
}
unsafe impl<T: StaticTypeSized> StaticType for dyn core::future::Future<Output = T> + Send + Sync + '_ {
	type Static = dyn core::future::Future<Output = T::Static> + Send + Sync;
}
unsafe impl<T: StaticTypeSized> StaticType for dyn core::future::Future<Output = T> + '_ {
	type Static = dyn core::future::Future<Output = T::Static>;
}
#[cfg(feature = "alloc")]
pub trait IntoDynAny<'n>: Sized + StaticType + 'n {
	fn into_dyn(self) -> Box<dyn DynAny<'n> + 'n> {
		Box::new(self)
	}
}
#[cfg(feature = "alloc")]
impl<'n, T: StaticType + 'n> IntoDynAny<'n> for T {}

#[cfg(feature = "alloc")]
impl From<()> for Box<dyn DynAny<'static>> {
	fn from(_: ()) -> Box<dyn DynAny<'static>> {
		Box::new(())
	}
}

#[cfg(feature = "alloc")]
use alloc::{
	borrow::Cow,
	boxed::Box,
	collections::{BTreeMap, BTreeSet, BinaryHeap, LinkedList, VecDeque},
	string::String,
	vec::Vec,
};
use core::sync::atomic::*;
use core::{
	cell::{Cell, RefCell, UnsafeCell},
	iter::Empty,
	marker::{PhantomData, PhantomPinned},
	mem::{ManuallyDrop, MaybeUninit},
	num::Wrapping,
	ops::Range,
	pin::Pin,
	time::Duration,
};

impl_type!(
	Option<T>, Result<T, E>, Cell<T>, UnsafeCell<T>, RefCell<T>, MaybeUninit<T>,
	 ManuallyDrop<T>, PhantomData<T>, PhantomPinned, Empty<T>, Range<T>,
	Wrapping<T>, Pin<T>, Duration, bool, f32, f64, char,
	u8, AtomicU8, u16, AtomicU16, u32, AtomicU32, u64,  usize, AtomicUsize,
	i8, AtomicI8, i16, AtomicI16, i32, AtomicI32, i64,  isize, AtomicIsize,
	i128, u128, AtomicBool, AtomicPtr<T>
);
#[cfg(feature = "large-atomics")]
impl_type!(AtomicU64, AtomicI64);

#[cfg(feature = "alloc")]
impl_type!(
	Vec<T>, String, BTreeMap<K,V>,BTreeSet<V>, LinkedList<T>, VecDeque<T>,
	BinaryHeap<T>
);

#[cfg(feature = "std")]
use std::sync::*;

#[cfg(feature = "std")]
impl_type!(Once, Mutex<T>, RwLock<T>);

#[cfg(feature = "rc")]
use std::rc::Rc;
#[cfg(feature = "rc")]
impl_type!(Rc<T>);
#[cfg(all(feature = "rc", feature = "alloc"))]
use std::sync::Arc;
#[cfg(all(feature = "rc", feature = "alloc"))]
unsafe impl<T: StaticType + ?Sized> StaticType for Arc<T> {
	type Static = Arc<<T as StaticType>::Static>;
}

#[cfg(feature = "glam")]
use glam::*;
#[cfg(feature = "glam")]
#[rustfmt::skip]
impl_type!(
	IVec2, IVec3, IVec4, UVec2, UVec3, UVec4, BVec2, BVec3, BVec4,
	Vec2, Vec3, Vec3A, Vec4, DVec2, DVec3, DVec4,
	Mat2, Mat3, Mat3A, Mat4, DMat2, DMat3, DMat4,
	Quat, Affine2, Affine3A, DAffine2, DAffine3, DQuat
);

#[cfg(feature = "alloc")]
unsafe impl<T: crate::StaticType + ?Sized> crate::StaticType for Box<T> {
	type Static = Box<<T as crate::StaticType>::Static>;
}
#[test]
fn test_tuple_of_boxes() {
	let tuple = (Box::new(&1 as &dyn DynAny<'static>), Box::new(&2 as &dyn DynAny<'static>));
	let dyn_any = &tuple as &dyn DynAny;
	assert_eq!(&1, downcast_ref(*downcast_ref::<(Box<&dyn DynAny>, Box<&dyn DynAny>)>(dyn_any).unwrap().0).unwrap());
	assert_eq!(&2, downcast_ref(*downcast_ref::<(Box<&dyn DynAny>, Box<&dyn DynAny>)>(dyn_any).unwrap().1).unwrap());
}

macro_rules! impl_tuple {
	(@rec $t:ident) => { };
	(@rec $_:ident $($t:ident)+) => {
		impl_tuple! { @impl $($t)* }
		impl_tuple! { @rec $($t)* }
	};
	(@impl $($t:ident)*) => {
		unsafe impl< $($t: StaticTypeSized,)*> StaticType for ($($t,)*) {
			type Static = ($(<$t as $crate::StaticTypeSized>::Static,)*);
		}
	};
	($($t:ident)*) => {
		impl_tuple! { @rec _t $($t)* }
	};
}

impl_tuple! {
	A B C D E F G H I J K L
}

#[test]
fn simple_downcast() {
	let x = Box::new(3_u32) as Box<dyn DynAny>;
	assert_eq!(*downcast::<u32>(x).unwrap(), 3_u32);
}
#[test]
#[should_panic]
fn simple_downcast_panic() {
	let x = Box::new(3_i32) as Box<dyn DynAny>;
	assert_eq!(*downcast::<u32>(x).expect("attempted to perform invalid downcast"), 3_u32);
}
