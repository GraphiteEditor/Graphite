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
pub enum Message {
	Foo,
	Bar(usize),
	Qux {
		x: usize,
	},
	#[child]
	Child(Child),
}

#[impl_message(Message, Child)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Child {
	Foo,
	Bar(usize),
	#[child]
	SubChild(Child2),
}

#[impl_message(Message, Child, SubChild)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Child2 {
	Foo,
	Bar,
}

fn main() {
	let c3 = Child2::Foo;
	assert_eq!(Message::from(c3), Message::Child(Child::SubChild(c3)));
	assert_eq!(
		MessageDiscriminant::from(Child2Discriminant::from(&c3)),
		MessageDiscriminant::Child(ChildDiscriminant::SubChild(Child2Discriminant::Foo))
	);
	println!("{}", Child2::Bar.to_discriminant().global_name());
}
