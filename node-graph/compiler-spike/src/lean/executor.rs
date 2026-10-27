//! Executor construction over the IR: one boxed node per IR node, wired to its inputs by index.

use super::Compiled;
use super::ir::IrValue;

pub trait MockNode: Send + Sync {
	fn input_count(&self) -> usize;
}

struct ConstructedNode {
	inputs: (u32, u32),
	_implementation: u32,
}

impl MockNode for ConstructedNode {
	fn input_count(&self) -> usize {
		(self.inputs.1 - self.inputs.0) as usize
	}
}

struct ValueNode(#[expect(dead_code, reason = "the mock only models the allocation, it never evaluates")] IrValue);

impl MockNode for ValueNode {
	fn input_count(&self) -> usize {
		0
	}
}

struct AdapterNode {
	_input: u32,
}

impl MockNode for AdapterNode {
	fn input_count(&self) -> usize {
		1
	}
}

pub struct Executor {
	pub nodes: Vec<Box<dyn MockNode>>,
	pub adapters: Vec<Box<dyn MockNode>>,
	pub output: u32,
}

impl Executor {
	pub fn new(compiled: &Compiled) -> Self {
		let (ir, typing) = (compiled.ir, compiled.typing);
		let mut nodes: Vec<Box<dyn MockNode>> = Vec::with_capacity(ir.len());
		let mut adapters: Vec<Box<dyn MockNode>> = Vec::new();

		for (id, node) in ir.nodes.iter().enumerate() {
			if let Some(value) = node.value {
				nodes.push(Box::new(ValueNode(value)));
				continue;
			}
			for promotion in typing.promotions(id as u32) {
				let input = ir.inputs[(node.inputs.0 + promotion.input) as usize];
				adapters.push(Box::new(AdapterNode { _input: input }));
			}
			nodes.push(Box::new(ConstructedNode {
				inputs: node.inputs,
				_implementation: typing.info[id].implementation,
			}));
		}

		Self {
			nodes,
			adapters,
			output: compiled.output,
		}
	}
}
