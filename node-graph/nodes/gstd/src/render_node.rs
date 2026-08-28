use core_types::gpoll::Interrupt;
use core_types::list::List;
use core_types::transform::{Footprint, Transform};
use core_types::{Color, Context, Ctx, DeriveCtx, ExtractFootprint, ExtractIndex, ExtractVarArgs, InjectIndex, VarArgLink, VarArgSlots, WasmNotSend};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graphene_application_io::{ExportFormat, RenderConfig};
use graphic_types::raster_types::{CPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::{Render, RenderMetadata, RenderOutputType as RenderOutputTypeRequest, RenderParams, SvgRender, SvgRenderOutput};
use std::sync::Arc;
use vector_types::GradientStops;
use wgpu_executor::RenderContext;

#[derive(Clone, dyn_any::DynAny)]
pub enum RenderIntermediateType {
	Vello(Arc<(vello::Scene, RenderContext)>),
	Svg(Arc<SvgRenderOutput>),
}
#[derive(Clone, dyn_any::DynAny)]
pub struct RenderIntermediate {
	pub(crate) ty: RenderIntermediateType,
	pub(crate) metadata: RenderMetadata,
}

fn intermediate_of<R: Render>(data: &R, render_params: &RenderParams) -> RenderIntermediate {
	let footprint = Footprint::default();
	let mut metadata = RenderMetadata::default();
	data.collect_metadata(&mut metadata, footprint, None);
	match &render_params.render_output_type {
		RenderOutputTypeRequest::Vello => {
			let mut scene = vello::Scene::new();

			let mut context = wgpu_executor::RenderContext::default();
			data.render_to_vello(&mut scene, Default::default(), &mut context, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Vello(Arc::new((scene, context))),
				metadata,
			}
		}
		RenderOutputTypeRequest::Svg => {
			let mut render = SvgRender::new();

			data.render_svg(&mut render, render_params);

			RenderIntermediate {
				ty: RenderIntermediateType::Svg(Arc::new(render.into())),
				metadata,
			}
		}
	}
}

#[node_macro::node(category(""))]
fn render_intermediate<T: dyn_any::StaticTypeSized + 'static + Render + WasmNotSend + Send + Sync>(
	ctx: impl Ctx + ExtractVarArgs + DeriveCtx,
	#[implementations(
		Context -> List<Artboard>,
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
		Context -> List<String>,
	)]
	data: impl Node<Context<'_>, Output = T>,
) -> Result<RenderIntermediate, Interrupt> {
	let data = data.eval(&ctx.derived())?;
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	Ok(intermediate_of(&data, render_params))
}

/// The leveled form of `render_intermediate`: the wire's records materialize
/// into a run, which renders directly.
#[node_macro::node(category(""))]
fn render_intermediate_leveled<T: Clone + Send + Sync + core_types::CacheHash + dyn_any::StaticTypeSized + 'static>(
	ctx: impl Ctx + ExtractVarArgs + ExtractIndex + InjectIndex + Copy,
	#[implementations(Artboard, Graphic, Vector, Raster<CPU>, Color, GradientStops, String)] data: IList<T>,
) -> Result<RenderIntermediate, Interrupt>
where
	for<'a> core_types::record::RunView<'a, T>: Render,
{
	let item = data.as_group_item();
	let run = core_types::record::RunView::<T>::new(&item).expect("the run holds the row's element type");
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");

	Ok(intermediate_of(&run, render_params))
}

#[node_macro::node(category(""))]
fn render(
	ctx: impl Ctx + ExtractFootprint + ExtractVarArgs,
	#[scope(crate::platform_application_io::try_wgpu_executor::IDENTIFIER)] executor: Option<wgpu_executor::WgpuExecutorHandle>,
	data: RenderIntermediate,
) -> RenderOutput {
	let footprint = ctx.footprint();
	let render_params = ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderParams>()
		.expect("Downcasting render params yielded invalid type");
	let mut render_params = render_params.clone();
	render_params.footprint = *footprint;

	let RenderIntermediate { ty, mut metadata } = data;
	metadata.apply_transform(footprint.transform);

	let data = match (render_params.render_output_type, ty) {
		(RenderOutputTypeRequest::Svg, RenderIntermediateType::Svg(data)) => {
			let logical_transform = glam::DAffine2::from_scale(glam::DVec2::splat(1.0 / render_params.scale)) * footprint.transform;
			let logical_resolution = footprint.resolution.as_dvec2() / render_params.scale;

			let mut render = SvgRender::from(data.as_ref());
			render.wrap_with_transform(logical_transform, Some(logical_resolution));

			let output = SvgRenderOutput::from(render);
			assert!(output.svg_defs.is_empty());

			RenderOutputType::Svg {
				svg: output.svg,
				image_data: output.image_data.into_iter().map(|(image, id)| (id, image.0)).collect(),
			}
		}
		(RenderOutputTypeRequest::Vello, RenderIntermediateType::Vello(data)) => {
			let (scene, context) = data.as_ref();

			let footprint_transform_vello = vello::kurbo::Affine::new(footprint.transform.to_cols_array());

			let mut transformed_scene = vello::Scene::new();
			transformed_scene.append(scene, Some(footprint_transform_vello));

			// We now replace all transforms which are supposed to be infinite with a transform which covers the entire viewport.
			// See <https://xi.zulipchat.com/#narrow/channel/197075-vello/topic/Full.20screen.20color.2Fgradients/near/538435044> for more detail.
			//
			// `!is_finite()` rather than `== f32::INFINITY`: `scene.append` composes the child's `Affine::scale(INFINITY)` with
			// the viewport rotation, leaving `matrix[0] = cos(θ) * INFINITY`. In the (90°, 270°) tilt range cos is negative so
			// the result is `-INFINITY`, which the old equality check missed; Vello then rasterized a unit rect with non-finite
			// vertices, dropping the gradient and tanking performance. `!is_finite()` also covers NaN as a guard against future
			// code paths where `matrix[0]` could land on `0 * INFINITY`.
			let scaled_infinite_transform = vello::kurbo::Affine::scale_non_uniform(footprint.resolution.x as f64, footprint.resolution.y as f64);
			for transform in transformed_scene.encoding_mut().transforms.iter_mut() {
				if !transform.matrix[0].is_finite() {
					*transform = vello_encoding::Transform::from_kurbo(&scaled_infinite_transform);
				}
			}

			let texture = executor
				.expect("GPU executor not available")
				.render_vello_scene(&transformed_scene, footprint.resolution, context, None)
				.expect("Failed to render Vello scene");
			RenderOutputType::Texture(texture)
		}
		_ => unreachable!("Render node did not receive its requested data type"),
	};

	RenderOutput { data, metadata }
}

#[node_macro::node(category(""))]
fn create_context(ctx: impl Ctx + ExtractVarArgs + DeriveCtx, data: impl Node<Context<'_>, Output = RenderOutput>) -> Result<RenderOutput, Interrupt> {
	let render_config = *ctx
		.vararg(0)
		.expect("Did not find var args")
		.downcast_ref::<RenderConfig>()
		.expect("Downcasting render config yielded invalid type");

	let render_output_type = match render_config.export_format {
		ExportFormat::Svg => RenderOutputTypeRequest::Svg,
		ExportFormat::Raster => RenderOutputTypeRequest::Vello,
	};

	let logical_viewport = render_config.viewport;
	let footprint = Footprint {
		transform: glam::DAffine2::from_scale(glam::DVec2::splat(render_config.scale)) * logical_viewport.transform,
		..logical_viewport
	};

	let render_params = RenderParams {
		render_mode: render_config.render_mode,
		for_export: render_config.for_export,
		render_output_type,
		scale: render_config.scale,
		viewport_zoom: logical_viewport.scale_magnitudes().x,
		..Default::default()
	};

	let scope = ctx
		.scope()
		.with_real_time(Some(render_config.time.time))
		.with_animation_time(Some(render_config.time.animation_time.as_secs_f64()))
		.with_pointer_position(Some(render_config.pointer));
	let varargs = VarArgLink {
		args: VarArgSlots::Single(&render_params),
		outer: None,
	};
	let scoped = ctx.with_scope(&scope);
	let with_params = scoped.with_varargs(&varargs);
	let mut result = data.eval(&with_params.with_footprint(&footprint))?;

	result.metadata.apply_transform(glam::DAffine2::from_scale(glam::DVec2::splat(1. / render_config.scale)));
	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::arena::Arena;
	use core_types::context::{ContextImpl, EvalScope, VarArgsResult};
	use core_types::gpoll::GPoll;
	use core_types::node::Node;
	use core_types::{ExtractAnimationTime, ExtractPointerPosition, ExtractRealTime};
	use graphene_application_io::TimingInformation;

	struct ProbeNode;

	impl<'a> Node<ContextImpl<'a>> for ProbeNode {
		type Output = RenderOutput;

		fn eval(&self, ctx: &ContextImpl<'a>) -> GPoll<RenderOutput> {
			let render_params = ctx.vararg(0).unwrap().downcast_ref::<RenderParams>().expect("the vararg chain must start with RenderParams");
			assert_eq!(render_params.scale, 2.0);
			assert!(matches!(ctx.vararg(1), Err(VarArgsResult::IndexOutOfBounds)), "the RenderConfig must not leak downstream");
			assert_eq!(ctx.footprint().transform, glam::DAffine2::from_scale(glam::DVec2::splat(2.0)) * Footprint::DEFAULT.transform);
			assert_eq!(ctx.try_real_time(), Some(1.5));
			assert_eq!(ctx.try_animation_time(), Some(2.0));
			assert_eq!(ctx.try_pointer_position(), Some(glam::DVec2::new(3.0, 4.0)));
			GPoll::Final(RenderOutput {
				data: RenderOutputType::Buffer {
					data: Vec::new(),
					width: 0,
					height: 0,
				},
				metadata: RenderMetadata::default(),
			})
		}
	}

	#[test]
	fn create_context_builds_the_render_context_from_the_root_vararg() {
		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = EvalScope::new(None, None, None, &generations, &arena);
		let root = ContextImpl::root(&scope);
		let render_config = RenderConfig {
			scale: 2.0,
			time: TimingInformation {
				time: 1.5,
				animation_time: std::time::Duration::from_secs(2),
			},
			pointer: glam::DVec2::new(3.0, 4.0),
			..Default::default()
		};
		let varargs = VarArgLink {
			args: VarArgSlots::Single(&render_config),
			outer: None,
		};
		let ctx = root.with_varargs(&varargs);

		let probe = core_types::record::RecordLift::<RenderOutput, _>::new(ProbeNode);
		let layout = Node::<ContextImpl>::layout(&probe).clone();
		// SAFETY: between evaluations, nothing served on the stack is live.
		unsafe { core_types::record::stack::reserve(layout.frame_bytes().max(1 << 12)); }		let mut graph = CreateContextNode::new(probe, &layout);
		// The executor resolves and installs the node's own layout at wiring;
		// without it the flip tail writes through the default empty layout.
		Node::<ContextImpl>::set_layout(
			&mut graph,
			core_types::record::RecordLayout {
				frame_bytes: layout.frame_bytes(),
				plan: Vec::new(),
				layout: layout.clone(),
				lane_invariant: u32::MAX,
			},
		);
		let GPoll::Final(result) = Node::<ContextImpl>::eval(&graph, &ctx) else {
			panic!("create_context must complete synchronously");
		};
		let output: &RenderOutput = unsafe { core_types::record::borrow_element(layout.rec(&result)) };
		assert_eq!(
			output.data,
			RenderOutputType::Buffer {
				data: Vec::new(),
				width: 0,
				height: 0
			}
		);
	}
}
