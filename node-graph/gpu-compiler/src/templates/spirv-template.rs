#![no_std]
#![feature(unchecked_math)]

#[cfg(target_arch = "spirv")]
extern crate spirv_std;

//#[cfg(target_arch = "spirv")]
//pub mod gpu {
//use super::*;
	use spirv_std::spirv;
	use spirv_std::glam;
	use spirv_std::glam::{UVec3, Vec2, Mat2, BVec2};

	#[allow(unused)]
	#[spirv(compute(threads({{compute_threads}})))]
	pub fn eval (
		#[spirv(global_invocation_id)] _global_index: UVec3,
		{% for input in inputs %}
		{{input}},
		{% endfor %}
	) {
		use graphene_core::{Node, NodeMut};
		use graphene_core::raster::adjustments::{BlendMode, BlendNode};
		use graphene_core::Color;

		{% for input in input_nodes %}
		let _i{{input.index}} = graphene_core::value::CopiedNode::new(*i{{input.index}});
		let _{{input.id}} = {{input.fqn}}::new({% for arg in input.args %}{{arg}}, {% endfor %});
		let {{input.id}} = graphene_core::structural::ComposeNode::new(_i{{input.index}}, _{{input.id}});
		{% endfor %}

		{% for node in nodes %}
		let mut {{node.id}} = {{node.fqn}}::new({% for arg in node.args %}{{arg}}, {% endfor %});
		{% endfor %}

		{% for output in output_nodes %}
		let v = {{output}}.eval(());
		o{{loop.index0}}[(_global_index.y * i0 + _global_index.x) as usize] = v;
		{% endfor %}
		// TODO: Write output to buffer
	}
//}
