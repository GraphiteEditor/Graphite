use crate::document::NodeNetwork;
use crate::graphene_compiler::Compiler;
use crate::proto::ProtoNetwork;

pub fn compile(network: NodeNetwork) -> ProtoNetwork {
	let compiler = Compiler {};
	compiler.compile_single(network).unwrap().network
}
