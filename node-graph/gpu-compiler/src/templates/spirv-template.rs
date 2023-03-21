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
        {% for input in inputs %}
        {{input}}
        {% endfor %}
	) {
		use graphene_core::Node;

        {$ for input in inputs %}
        let i{{loop.index0}} = graphene_core::value::CopiedNode::new({{input}});
		let _{{input.id}} = {{input.fqn}}::new({% for arg in input.args %}{{arg}}, {% endfor %});
        let {{input.id}} = graphene_core::structural::ComposeNode::new(i{{loop.index0}}, _{{input.id}}));
        {% endfor %}

		{% for node in nodes %}
		let {{node.id}} = {{node.fqn}}::new({% for arg in node.args %}{{arg}}, {% endfor %});
		{% endfor %}
		{{last_node}}.eval(input)
	}
}
