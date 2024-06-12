pub mod easing;
pub mod keyframe;

use keyframe::KeyframesF64;

use crate::application_io::EditorApi;
use crate::Node;

#[derive(Debug, Copy, Clone)]
pub struct AnimationF64Node<Keyframes> {
	keyframes: Keyframes,
}

#[node_macro::node_fn(AnimationF64Node)]
fn animation_f64_node<'a: 'input, T>(editor: EditorApi<'a, T>, keyframes: KeyframesF64) -> f64 {
	keyframes.get_value_at_time(editor.render_config.animation_config.time)
}
