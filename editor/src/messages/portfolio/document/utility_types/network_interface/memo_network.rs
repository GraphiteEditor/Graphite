use graph_craft::document::NodeNetwork;
use std::cell::Cell;
use std::hash::{Hash, Hasher};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct MemoNetwork {
	network: NodeNetwork,
	hash_code: Cell<Option<u64>>,
}

impl<'de> serde::Deserialize<'de> for MemoNetwork {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(Self::new(NodeNetwork::deserialize(deserializer)?))
	}
}

impl serde::Serialize for MemoNetwork {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		self.network.serialize(serializer)
	}
}

impl Hash for MemoNetwork {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.current_hash().hash(state);
	}
}

impl MemoNetwork {
	pub fn network(&self) -> &NodeNetwork {
		&self.network
	}

	pub fn network_mut(&mut self) -> &mut NodeNetwork {
		self.hash_code.set(None);
		&mut self.network
	}

	pub fn new(network: NodeNetwork) -> Self {
		Self { network, hash_code: None.into() }
	}

	pub fn current_hash(&self) -> u64 {
		let mut hash_code = self.hash_code.get();
		if hash_code.is_none() {
			hash_code = Some(self.network.current_hash());
			self.hash_code.set(hash_code);
		}
		hash_code.unwrap()
	}
}
