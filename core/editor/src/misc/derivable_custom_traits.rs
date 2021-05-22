//! Traits that can be derived using macros from `graphite-proc-macros`

use std::collections::HashMap;

pub trait Hint {
	fn hints(&self) -> HashMap<String, String>;
}

pub trait ToDiscriminant {
	type Discriminant;

	fn to_discriminant(&self) -> Self::Discriminant;
}

pub trait TransitiveChild: Into<Self::Parent> + Into<Self::TopParent> {
	type TopParent;
	type Parent;
}
