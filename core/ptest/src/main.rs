use graphite_proc_macros::AsMessage;
use std::fmt::Display;

//trait AsMessage: Sized + Into<Message> + Send + Sync + PartialEq<Message> + Display {
trait AsMessage: Sized + Send + Sync + Into<Message> + Display + PartialEq<Message> {
	//trait AsMessage: Sized + Send + Sync {
	//trait AsMessage: Sized + Send + Sync + Into<Message> + Display {
	fn name(&self) -> String;
	fn suffix(&self) -> &'static str;
	fn prefix() -> String;
}

#[derive(PartialEq, Clone)]
enum Message {
	Child(Child),
}

impl AsMessage for Message {
	fn prefix() -> String {
		"".into()
	}
	fn suffix(&self) -> &'static str {
		match self {
			Self::Child(c) => "Child",
		}
	}
	fn name(&self) -> String {
		".Child".into()
	}
}

impl Display for Message {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}", stringify!(self))
	}
}

#[derive(AsMessage, PartialEq, Clone)]
#[message(Message, Message, Child)]
enum Child {
	Document(DocumentMessage),
}

#[derive(AsMessage, PartialEq, Clone)]
#[message(Message, Child, Document)]
enum DocumentMessage {
	Foo,
	Bar,
}

fn main() {
	println!("Hello, world!");
}
