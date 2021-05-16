use graphite_proc_macros::*;

pub trait AsMessage: TransitiveChild
where
	Self::TopParent: TransitiveChild<Parent = Self::TopParent, TopParent = Self::TopParent> + AsMessage,
{
	fn local_name(self) -> String;
	fn global_name(self) -> String {
		<Self as Into<Self::TopParent>>::into(self).local_name()
	}
}

pub trait ToDiscriminant {
	type Discriminant;

	fn to_discriminant(&self) -> Self::Discriminant;
}

pub trait TransitiveChild: Into<Self::Parent> + Into<Self::TopParent> {
	type TopParent;
	type Parent;
}

#[impl_message]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Message2 {
	Foo,
	Bar(usize),
	Qux {
		x: usize,
	},
	#[child]
	Child(Child2),
}

impl TransitiveChild for Message2 {
	type TopParent = Self;
	type Parent = Self;
}

impl TransitiveChild for Message2Discriminant {
	type TopParent = Self;
	type Parent = Self;
}

#[impl_message(Message2, Child)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Child2 {
	Foo,
	Bar(usize),
	#[child]
	SubChild(Child3),
}

#[impl_message(Message2, Child2, SubChild)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Child3 {
	Foo,
	Bar,
}

fn main() {
	let c3 = Child3::Foo;
	assert_eq!(Message2::from(c3), Message2::Child(Child2::SubChild(c3)));
	assert_eq!(
		Message2Discriminant::from(Child3Discriminant::from(&c3)),
		Message2Discriminant::Child(Child2Discriminant::SubChild(Child3Discriminant::Foo))
	);
	println!("{}", Child3::Bar.to_discriminant().global_name());
}
