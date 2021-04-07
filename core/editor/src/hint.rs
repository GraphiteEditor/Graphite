use std::collections::HashMap;

pub trait Hint {
	fn hints(&self) -> HashMap<String, String>;
}
