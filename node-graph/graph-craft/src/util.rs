use crate::document::NodeNetwork;
use crate::graphene_compiler::Compiler;
use crate::proto::ProtoNetwork;

pub fn compile(network: NodeNetwork) -> ProtoNetwork {
	let compiler = Compiler {};
	compiler.compile_single(network).unwrap().network
}

pub static DEMO_ART: [&str; 7] = [
	"changing-seasons",
	"painted-dreams",
	"red-dress",
	"valley-of-spires",
	"isometric-fountain",
	"procedural-string-lights",
	"parametric-dunescape",
];
