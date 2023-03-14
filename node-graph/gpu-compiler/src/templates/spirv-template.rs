#![no_std]
#![feature(unchecked_math)]
#![deny(warnings)]

#[cfg(target_arch = "spirv")]
extern crate spirv_std;

#[cfg(target_arch = "spirv")]
pub mod gpu {
	use super::*;
	use spirv_std::spirv;
	use spirv_std::glam::UVec3;

	#[allow(unused)]
	#[spirv(compute(threads({{compute_threads}})))]
	pub fn eval (
		#[spirv(global_invocation_id)] global_id: UVec3,
		#[spirv(storage_buffer, descriptor_set = 0, binding = 0)] a: &[{{input_type}}],
		#[spirv(storage_buffer, descriptor_set = 0, binding = 1)] y: &mut [{{output_type}}],
		//#[spirv(push_constant)] push_consts: &graphene_core::gpu::PushConstants,
	) {
		let gid = global_id.x as usize;
		// Only process up to n, which is the length of the buffers.
		//if global_id.x < push_consts.n {
			y[gid] = node_graph(a[gid]);
		//}
	}

	fn node_graph(input: {{input_type}}) -> {{output_type}} {
		use graphene_core::Node;

		{% for node in nodes %}
		let {{node.id}} = {{node.fqn}}::new({% for arg in node.args %}{{arg}}, {% endfor %});
		{% endfor %}
		{{last_node}}.eval(input)
	}

}
