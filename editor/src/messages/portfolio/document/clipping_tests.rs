//! Layers clipped to a base through the editor's own graph, where the base's filters shape what clips to it but the shadows it casts don't.

use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
use crate::test_utils::gpu_executor;
use crate::test_utils::test_prelude::*;
use glam::{DAffine2, UVec2};
use graph_craft::document::value::TaggedValue;
use graphene_std::blending_nodes::effects::{blur, drop_shadow};
use graphene_std::graphic::{IntoGraphicList, extend};
use graphene_std::list::{Item, List};
use graphene_std::renderer::{Render, RenderParams, RenderSvgSegmentList, SvgRender};
use graphene_std::{Graphic, NodeParameter, ProtoNodeIdentifier, Vector};

#[derive(Clone, Copy, Debug)]
enum Base {
	VectorLayer,
	ImageLayer,
	/// A group holding a vector layer, with the effect on the group.
	Group,
}

async fn add_effect(editor: &mut EditorTestUtils, layer: LayerNodeIdentifier, effect: ProtoNodeIdentifier) {
	let node_type = DefinitionIdentifier::ProtoNode(effect.clone());
	editor.handle_message(NodeGraphMessage::CreateNodeInLayerWithTransaction { node_type, layer }).await;

	if effect == blur::IDENTIFIER {
		let node_id = NodeGraphLayer::new(layer, &editor.active_document().network_interface)
			.upstream_node_id_from_protonode(effect)
			.expect("the layer has its blur");
		let value = Box::new(TaggedValue::Number(10.));
		editor
			.handle_message(NodeGraphMessage::SetInputValue {
				node_id,
				input_index: blur::RadiusInput::INDEX,
				value,
			})
			.await;
	}
}

/// The root stack of a document with a blue base filling (50, 50) to (130, 130) and a larger red layer clipped to it, with `effect` on the base.
async fn clipped_stack(base: Base, effect: ProtoNodeIdentifier) -> List<Graphic> {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;

	let blue = Color::from_rgbaf32_unchecked(0., 0., 1., 1.);
	let red = Color::from_rgbaf32_unchecked(1., 0., 0., 1.);

	match base {
		Base::ImageLayer => editor.create_raster_image(Image::new(80, 80, blue), Some((90., 90.))).await,
		Base::VectorLayer | Base::Group => {
			editor.select_primary_color(blue).await;
			editor.select_secondary_color(blue).await;
			editor.draw_rect(50., 50., 130., 130.).await;
		}
	}
	let base_layer = editor.get_selected_layer().await.expect("the base is selected");
	if !matches!(base, Base::Group) {
		add_effect(&mut editor, base_layer, effect.clone()).await;
	}

	editor.select_tool(ToolType::Select).await;
	editor.select_primary_color(red).await;
	editor.select_secondary_color(red).await;
	editor.draw_rect(20., 20., 200., 200.).await;
	let clipped_layer = editor.get_selected_layer().await.expect("the clipped layer is selected");
	editor
		.handle_message(GraphOperationMessage::FillColorSet {
			layer: clipped_layer,
			color: Some(red),
		})
		.await;
	editor.handle_message(DocumentMessage::ClipLayer { id: clipped_layer.to_node() }).await;

	// Grouped once the red is clipped, the group takes the base's place beneath it
	if let Base::Group = base {
		let nodes = vec![base_layer.to_node()];
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes }).await;
		let group_folder_type = GroupFolderType::Layer;
		editor.handle_message(DocumentMessage::GroupSelectedLayers { group_folder_type }).await;
		let group = editor.get_selected_layer().await.expect("the group is selected");
		add_effect(&mut editor, group, effect).await;
	}

	// The stack is what the root's 'Extend' nodes put together: the base's layer extended by the clipped one
	let instrumented = editor.eval_graph_until_finished().await.expect("the graph evaluates");
	let mut news = instrumented.grab_all_input::<extend::NewInput, Item<Graphic>>(&editor.runtime);
	let clipped = news.find(|new| new.element().had_clip_enabled()).expect("the clipped layer is extended onto the base");
	let mut bases = instrumented.grab_all_input::<extend::BaseInput, List<Graphic>>(&editor.runtime);
	let mut stack = bases.find(|base| base.len() == 1).expect("the base's layer is extended");
	stack.push(clipped);
	stack
}

/// Checks in SVG and in Vello that the red clipped to the base shows past the base's edges if `blurred`, and that a shadow the base casts doesn't widen the clip.
fn assert_clip(name: &str, graphic: &Graphic, blurred: bool) {
	// The clip's def in SVG carries out the blur but not the shadow
	let mut render = SvgRender::new();
	graphic.render_svg(
		&mut render,
		&RenderParams {
			for_export: true,
			..Default::default()
		},
	);
	let body = render.svg.to_svg_string();
	let reference = body.rfind("url(#mask-").expect("the red is clipped");
	let id: String = body[reference + "url(#".len()..].chars().take_while(|&character| character != ')').collect();
	let defs = &render.svg_defs;
	let def_start = defs.find(&format!(r#"id="{id}""#)).expect("the clip has a def");
	let def_end = ["</mask>", "</clipPath>"]
		.iter()
		.filter_map(|closing| defs[def_start..].find(closing))
		.min()
		.expect("the clip's def is closed");
	let clip = &defs[def_start..def_start + def_end];
	assert_eq!(clip.contains("filter="), blurred, "{name}: the clip's def is {clip}");

	// In Vello, the red past the base's edges shows where the blur reaches, but not where the shadow falls
	let size = UVec2::new(256, 256);
	let executor = gpu_executor();
	let texture = executor
		.render_vello(size, DAffine2::IDENTITY, &mut |scene| graphic.render_to_vello(scene, DAffine2::IDENTITY, &RenderParams::default()))
		.expect("the clip renders");
	let pixels = executor.read_texture_pixels(&texture).expect("the render reads back");
	let pixel = |x: u32, y: u32| -> [u8; 4] {
		let index = (y * size.x + x) as usize * 4;
		[pixels[index], pixels[index + 1], pixels[index + 2], pixels[index + 3]]
	};

	let within = pixel(90, 90);
	assert!(within[0] >= 250 && within[2] <= 5 && within[3] == 255, "{name}: within the base, the red is {within:?}");
	assert_eq!(pixel(134, 90)[0] > 0, blurred, "{name}: past the base's right edge, the red is {:?}", pixel(134, 90));
	assert_eq!(pixel(93, 135)[0] > 0, blurred, "{name}: past the base's bottom edge, the red is {:?}", pixel(93, 135));
	assert!(blurred || pixel(93, 135)[3] > 0, "{name}: the base's shadow shows where it falls");
	assert_eq!(pixel(170, 90)[3], 0, "{name}: the red is clipped away well past the base");
}

#[tokio::test]
async fn clipping_bases_clip_by_their_filters_but_not_the_shadows_they_cast() {
	for (effect_name, effect, blurred) in [("Blur", blur::IDENTIFIER, true), ("Drop Shadow", drop_shadow::IDENTIFIER, false)] {
		for base in [Base::VectorLayer, Base::ImageLayer, Base::Group] {
			let stack = clipped_stack(base, effect.clone()).await;
			assert_clip(&format!("{base:?} with {effect_name}"), &Graphic::GraphicList(stack.clone()), blurred);

			// Flattened into one list of vectors, the base clips its sibling by the same rule
			if let Base::VectorLayer = base {
				let flattened: List<Vector> = stack.into_flattened_list();
				assert_clip(&format!("flattened vectors with {effect_name}"), &Graphic::VectorList(flattened), blurred);
			}
		}
	}
}
