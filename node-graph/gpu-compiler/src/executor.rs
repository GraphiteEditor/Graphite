use std::error::Error;

use dyn_any::DynAny;

use crate::{document::NodeNetwork, proto::ProtoNetwork};

pub struct Compiler {}

impl Compiler {
	pub fn compile(&self, mut network: NodeNetwork, resolve_inputs: bool) -> ProtoNetwork {
		let node_count = network.nodes.len();
		println!("flattening");
		for id in 0..node_count {
			network.flatten(id as u64);
		}
		let mut proto_network = network.into_proto_network();
		if resolve_inputs {
			println!("resolving inputs");
			proto_network.resolve_inputs();
		}
		println!("reordering ids");
		proto_network.reorder_ids();
		proto_network
	}
}
pub type Any<'a> = Box<dyn DynAny<'a> + 'a>;

pub trait Executor {
	fn execute(&self, input: Any<'static>) -> Result<Any<'static>, Box<dyn Error>>;
}
