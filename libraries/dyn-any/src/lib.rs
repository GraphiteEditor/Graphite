#![doc(html_root_url = "http://docs.rs/const-default/1.0.0")]
#![cfg_attr(feature = "unstable-docs", feature(doc_cfg))]

#[cfg(feature = "derive")]
#[cfg_attr(feature = "unstable-docs", doc(cfg(feature = "derive")))]
pub use dyn_any_derive::DynAny;

use std::any::TypeId;

pub trait DynAny<'a> {
	fn type_id(&self) -> TypeId;
}

impl<'a, T: StaticType> DynAny<'a> for T {
	fn type_id(&self) -> std::any::TypeId {
		std::any::TypeId::of::<T::Static>()
	}
}
pub fn downcast_ref<'a, V: StaticType>(i: &'a dyn DynAny<'a>) -> Option<&'a V> {
	if i.type_id() == std::any::TypeId::of::<<V as StaticType>::Static>() {
		// SAFETY: caller guarantees that T is the correct type
		let ptr = i as *const dyn DynAny<'a> as *const V;
		Some(unsafe { &*ptr })
	} else {
		None
	}
}

pub fn downcast<'a, V: StaticType>(i: Box<dyn DynAny<'a> + 'a>) -> Option<Box<V>> {
	let type_id = DynAny::type_id(i.as_ref());
	if type_id == std::any::TypeId::of::<<V as StaticType>::Static>() {
		// SAFETY: caller guarantees that T is the correct type
		let ptr = Box::into_raw(i) as *mut dyn DynAny<'a> as *mut V;
		Some(unsafe { Box::from_raw(ptr) })
	} else {
		// TODO: add log error
		if type_id == std::any::TypeId::of::<&dyn DynAny<'static>>() {
			panic!("downcast error: type_id == std::any::TypeId::of::<dyn DynAny<'a>>()");
		}
		println!("expected: {:?}", std::any::TypeId::of::<<V as StaticType>::Static>());
		println!("actual one: {:?}", type_id);
		None
	}
}

pub trait StaticType {
	type Static: 'static + ?Sized;
	fn type_id(&self) -> std::any::TypeId {
		std::any::TypeId::of::<Self::Static>()
	}
}

pub trait StaticTypeSized {
	type Static: 'static;
	fn type_id(&self) -> std::any::TypeId {
		std::any::TypeId::of::<Self::Static>()
	}
}
impl<T: StaticType + Sized> StaticTypeSized for T
where
	T::Static: Sized,
{
	type Static = <T as StaticType>::Static;
}
pub trait StaticTypeClone {
	type Static: 'static + Clone;
	fn type_id(&self) -> std::any::TypeId {
		std::any::TypeId::of::<Self::Static>()
	}
}
impl<T: StaticType + Clone> StaticTypeClone for T
where
	T::Static: Clone,
{
	type Static = <T as StaticType>::Static;
}

macro_rules! impl_type {
    ($($id:ident$(<$($(($l:lifetime, $s:lifetime)),*|)?$($T:ident),*>)?),*) => {
        $(
        impl< $($($T:  $crate::StaticTypeSized ,)*)?> $crate::StaticType for $id $(<$($($l,)*)?$($T, )*>)?{
            type Static = $id$(<$($($s,)*)?$(<$T as $crate::StaticTypeSized>::Static,)*>)?;
        }
        )*
    };
}
impl<'a, T: StaticTypeClone + Clone> StaticType for std::borrow::Cow<'a, T> {
	type Static = std::borrow::Cow<'static, T::Static>;
}
impl<T: StaticTypeSized> StaticType for *const [T] {
	type Static = *const [<T as StaticTypeSized>::Static];
}
impl<T: StaticTypeSized> StaticType for *mut [T] {
	type Static = *mut [<T as StaticTypeSized>::Static];
}
impl<'a, T: StaticTypeSized> StaticType for &'a [T] {
	type Static = &'static [<T as StaticTypeSized>::Static];
}
impl<'a> StaticType for &'a str {
	type Static = &'static str;
}
impl StaticType for () {
	type Static = ();
}
impl<'a, T: 'a + StaticType + ?Sized> StaticType for &'a T {
	type Static = &'static <T as StaticType>::Static;
}
impl<T: StaticTypeSized, const N: usize> StaticType for [T; N] {
	type Static = [<T as StaticTypeSized>::Static; N];
}

impl<'a> StaticType for dyn DynAny<'a> + '_ {
	type Static = dyn DynAny<'static>;
}
pub trait IntoDynAny<'n>: Sized + StaticType + 'n {
	fn into_dyn(self) -> Box<dyn DynAny<'n> + 'n> {
		Box::new(self)
	}
}
impl<'n, T: StaticType + 'n> IntoDynAny<'n> for T {}

impl From<()> for Box<dyn DynAny<'static>> {
	fn from(_: ()) -> Box<dyn DynAny<'static>> {
		Box::new(())
	}
}

use core::{
	cell::{Cell, RefCell, UnsafeCell},
	iter::Empty,
	marker::{PhantomData, PhantomPinned},
	mem::{ManuallyDrop, MaybeUninit},
	num::Wrapping,
	time::Duration,
};
use std::{
	collections::*,
	sync::{atomic::*, *},
	vec::Vec,
};

impl_type!(Option<T>,Result<T, E>,Cell<T>,UnsafeCell<T>,RefCell<T>,MaybeUninit<T>,
		   Vec<T>, String, BTreeMap<K,V>,BTreeSet<V>, LinkedList<T>, VecDeque<T>,
		   BinaryHeap<T>,  ManuallyDrop<T>, PhantomData<T>, PhantomPinned,Empty<T>,
		   Wrapping<T>, Duration, Once, Mutex<T>, RwLock<T>,  bool, f32, f64, char,
		   u8, AtomicU8, u16,AtomicU16, u32,AtomicU32, u64,AtomicU64, usize,AtomicUsize,
		   i8,AtomicI8, i16,AtomicI16, i32,AtomicI32, i64,AtomicI64, isize,AtomicIsize,
			i128, u128, AtomicBool, AtomicPtr<T>
);

impl<T: crate::StaticType + ?Sized> crate::StaticType for Box<T> {
	type Static = Box<<T as crate::StaticType>::Static>;
}
fn test() {
	let foo = (Box::new(&1 as &dyn DynAny<'static>), Box::new(&2 as &dyn DynAny<'static>));
	//let bar = &foo as &dyn DynAny<'static>;
}

macro_rules! impl_tuple {
    (@rec $t:ident) => { };
    (@rec $_:ident $($t:ident)+) => {
        impl_tuple! { @impl $($t)* }
        impl_tuple! { @rec $($t)* }
    };
    (@impl $($t:ident)*) => {
        impl< $($t: StaticTypeSized,)*> StaticType for ($($t,)*) {
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
