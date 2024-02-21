use dyn_any::StaticTypeSized;
use glam::{DAffine2, DVec2, Mat2, Vec2};
use gpu_executor::{Bindgroup, ComputePassDimensions, PipelineLayout, StorageBufferOptions};
use gpu_executor::{GpuExecutor, ShaderIO, ShaderInput};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;
use graph_craft::proto::*;
use graphene_core::quantization::QuantizationChannels;
use graphene_core::raster::*;
use graphene_core::*;
use wgpu_executor::WgpuExecutor;

#[cfg(feature = "quantization")]
use graphene_core::quantization::PackedPixel;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::wasm_application_io::WasmApplicationIo;

pub struct GpuCompiler<TypingContext, ShaderIO> {
	typing_context: TypingContext,
	io: ShaderIO,
}

// TODO: Move to graph-craft
#[node_macro::node_fn(GpuCompiler)]
async fn compile_gpu(node: &'input DocumentNode, mut typing_context: TypingContext, io: ShaderIO) -> Result<compilation_client::Shader, String> {
	let compiler = graph_craft::graphene_compiler::Compiler {};
	let DocumentNodeImplementation::Network(ref network) = node.implementation else { panic!() };
	let proto_networks: Vec<_> = compiler.compile(network.clone())?.collect();

	for network in proto_networks.iter() {
		typing_context.update(network).expect("Failed to type check network");
	}
	// TODO: do a proper union
	let input_types = proto_networks[0]
		.inputs
		.iter()
		.map(|id| typing_context.type_of(*id).unwrap())
		.map(|node_io| node_io.output.clone())
		.collect();
	let output_types = proto_networks.iter().map(|network| typing_context.type_of(network.output).unwrap().output.clone()).collect();

	Ok(compilation_client::compile(proto_networks, input_types, output_types, io).await.unwrap())
}

pub struct MapGpuNode<Node, EditorApi> {
	node: Node,
	editor_api: EditorApi,
	cache: RefCell<HashMap<String, ComputePass<WgpuExecutor>>>,
}

struct ComputePass<T: GpuExecutor> {
	pipeline_layout: PipelineLayout<T>,
	readback_buffer: Option<Arc<ShaderInput<T>>>,
}

impl<T: GpuExecutor> Clone for ComputePass<T> {
	fn clone(&self) -> Self {
		Self {
			pipeline_layout: self.pipeline_layout.clone(),
			readback_buffer: self.readback_buffer.clone(),
		}
	}
}

#[node_macro::node_impl(MapGpuNode)]
async fn map_gpu<'a: 'input>(image: ImageFrame<Color>, node: DocumentNode, editor_api: graphene_core::application_io::EditorApi<'a, WasmApplicationIo>) -> ImageFrame<Color> {
	log::debug!("Executing gpu node");
	let executor = &editor_api.application_io.gpu_executor.as_ref().unwrap();

	#[cfg(feature = "quantization")]
	let quantization = crate::quantization::generate_quantization_from_image_frame(&image);
	#[cfg(not(feature = "quantization"))]
	let quantization = QuantizationChannels::default();
	log::debug!("quantization: {quantization:?}");

	#[cfg(feature = "image-compare")]
	let img: image::DynamicImage = image::Rgba32FImage::from_raw(image.image.width, image.image.height, bytemuck::cast_vec(image.image.data.clone()))
		.unwrap()
		.into();

	#[cfg(feature = "quantization")]
	let image = ImageFrame {
		image: Image {
			data: image.image.data.iter().map(|c| quantization::quantize_color(*c, quantization)).collect(),
			width: image.image.width,
			height: image.image.height,
			base64_string: None,
		},
		transform: image.transform,
		alpha_blending: image.alpha_blending,
	};

	// TODO: The cache should be based on the network topology not the node name
	let compute_pass_descriptor = if self.cache.borrow().contains_key(&node.name) {
		self.cache.borrow().get(&node.name).unwrap().clone()
	} else {
		let name = node.name.clone();
		let Ok(compute_pass_descriptor) = create_compute_pass_descriptor(node, &image, executor, quantization).await else {
			log::error!("Error creating compute pass descriptor in 'map_gpu()");
			return ImageFrame::empty();
		};
		self.cache.borrow_mut().insert(name, compute_pass_descriptor.clone());
		log::error!("created compute pass");
		compute_pass_descriptor
	};

	let compute_pass = executor
		.create_compute_pass(
			&compute_pass_descriptor.pipeline_layout,
			compute_pass_descriptor.readback_buffer.clone(),
			ComputePassDimensions::XY(image.image.width / 12 + 1, image.image.height / 8 + 1),
		)
		.unwrap();
	executor.execute_compute_pipeline(compute_pass).unwrap();
	log::debug!("executed pipeline");
	log::debug!("reading buffer");
	let result = executor.read_output_buffer(compute_pass_descriptor.readback_buffer.clone().unwrap()).await.unwrap();
	#[cfg(feature = "quantization")]
	let colors = bytemuck::pod_collect_to_vec::<u8, PackedPixel>(result.as_slice());
	#[cfg(feature = "quantization")]
	log::debug!("first color: {:b}", colors[0].0);
	#[cfg(feature = "quantization")]
	let colors: Vec<_> = colors.iter().map(|c| quantization::dequantize_color(*c, quantization)).collect();
	#[cfg(not(feature = "quantization"))]
	let colors = bytemuck::pod_collect_to_vec::<u8, Color>(result.as_slice());
	log::debug!("first color: {:?}", colors[0]);

	#[cfg(feature = "image-compare")]
	let img2: image::DynamicImage = image::Rgba32FImage::from_raw(image.image.width, image.image.height, bytemuck::cast_vec(colors.clone())).unwrap().into();
	#[cfg(feature = "image-compare")]
	let score = image_compare::rgb_hybrid_compare(&img.into_rgb8(), &img2.into_rgb8()).unwrap();
	#[cfg(feature = "image-compare")]
	log::debug!("score: {:?}", score.score);

	ImageFrame {
		image: Image {
			data: colors,
			width: image.image.width,
			height: image.image.height,
			..Default::default()
		},
		transform: image.transform,
		alpha_blending: image.alpha_blending,
	}
}

impl<Node, EditorApi> MapGpuNode<Node, EditorApi> {
	pub fn new(node: Node, editor_api: EditorApi) -> Self {
		Self {
			node,
			editor_api,
			cache: RefCell::new(HashMap::new()),
		}
	}
}

async fn create_compute_pass_descriptor<T: Clone + Pixel + StaticTypeSized>(
	node: DocumentNode,
	image: &ImageFrame<T>,
	executor: &&WgpuExecutor,
	quantization: QuantizationChannels,
) -> Result<ComputePass<WgpuExecutor>, String> {
	let compiler = graph_craft::graphene_compiler::Compiler {};
	let inner_network = NodeNetwork::value_network(node);

	log::debug!("inner_network: {inner_network:?}");
	let network = NodeNetwork {
		inputs: vec![NodeId(2), NodeId(1)], //vec![0, 1],
		#[cfg(feature = "quantization")]
		outputs: vec![NodeOutput::new(NodeId(5), 0)],
		#[cfg(not(feature = "quantization"))]
		outputs: vec![NodeOutput::new(NodeId(3), 0)],
		nodes: [
			DocumentNode {
				name: "Slice".into(),
				inputs: vec![NodeInput::Inline(InlineRust::new("i1[(_global_index.y * i0 + _global_index.x) as usize]".into(), concrete![Color]))],
				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::CopiedNode".into()),
				..Default::default()
			},
			DocumentNode {
				name: "Quantization".into(),
				inputs: vec![NodeInput::Network(concrete!(quantization::Quantization))],
				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into()),
				..Default::default()
			},
			DocumentNode {
				name: "Width".into(),
				inputs: vec![NodeInput::Network(concrete!(u32))],
				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into()),
				..Default::default()
			},
			/*DocumentNode {
				name: "Index".into(),
				//inputs: vec![NodeInput::Network(concrete!(UVec3))],
				inputs: vec![NodeInput::Inline(InlineRust::new("i1.x as usize".into(), concrete![u32]))],
				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::CopiedNode".into()),
				..Default::default()
			},*/
				/*
			DocumentNode {
				name: "GetNode".into(),
				inputs: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(0), 0)],
				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::storage::GetNode".into()),
				..Default::default()
			},*/
			#[cfg(feature = "quantization")]
			DocumentNode {
				name: "Dequantize".into(),
				inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(1), 0)],
				implementation: DocumentNodeImplementation::proto("graphene_core::quantization::DeQuantizeNode"),
				..Default::default()
			},
			DocumentNode {
				name: "MapNode".into(),
				#[cfg(feature = "quantization")]
				inputs: vec![NodeInput::node(NodeId(3), 0)],
				#[cfg(not(feature = "quantization"))]
				inputs: vec![NodeInput::node(NodeId(0), 0)],
				implementation: DocumentNodeImplementation::Network(inner_network),
				..Default::default()
			},
			#[cfg(feature = "quantization")]
			DocumentNode {
				name: "Quantize".into(),
				inputs: vec![NodeInput::node(NodeId(4), 0), NodeInput::node(NodeId(1), 0)],
				implementation: DocumentNodeImplementation::proto("graphene_core::quantization::QuantizeNode"),
				..Default::default()
			},
			/*
			DocumentNode {
				name: "SaveNode".into(),
				inputs: vec![
					NodeInput::node(NodeId(5), 0),
					NodeInput::Inline(InlineRust::new(
						"|x| o0[(_global_index.y * i1 + _global_index.x) as usize] = x".into(),
						//"|x|()".into(),
						Type::Fn(Box::new(concrete!(PackedPixel)), Box::new(concrete!(()))),
					)),
				],
				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::generic::FnMutNode".into()),
				..Default::default()
			},
			*/
		]
		.into_iter()
		.enumerate()
		.map(|(id, node)| (NodeId(id as u64), node))
		.collect(),
		..Default::default()
	};
	log::debug!("compiling network");
	let proto_networks = compiler.compile(network.clone())?.collect();
	log::debug!("compiling shader");
	let shader = compilation_client::compile(
		proto_networks,
		vec![concrete!(u32), concrete!(Color)], //, concrete!(u32)],
		vec![concrete!(Color)],
		ShaderIO {
			#[cfg(feature = "quantization")]
			inputs: vec![
				ShaderInput::UniformBuffer((), concrete!(u32)),
				ShaderInput::StorageBuffer((), concrete!(PackedPixel)),
				ShaderInput::UniformBuffer((), concrete!(quantization::QuantizationChannels)),
				//ShaderInput::Constant(gpu_executor::GPUConstant::GlobalInvocationId),
				ShaderInput::OutputBuffer((), concrete!(PackedPixel)),
			],
			#[cfg(not(feature = "quantization"))]
			inputs: vec![
				ShaderInput::UniformBuffer((), concrete!(u32)),
				ShaderInput::StorageBuffer((), concrete!(Color)),
				ShaderInput::OutputBuffer((), concrete!(Color)),
			],
			#[cfg(feature = "quantization")]
			output: ShaderInput::OutputBuffer((), concrete!(PackedPixel)),
			#[cfg(not(feature = "quantization"))]
			output: ShaderInput::OutputBuffer((), concrete!(Color)),
		},
	)
	.await
	.unwrap();
	//return ImageFrame::empty();
	let len: usize = image.image.data.len();

	/*
	let canvas = editor_api.application_io.create_surface();

	let surface = unsafe { executor.create_surface(canvas) }.unwrap();
	//log::debug!("id: {surface:?}");
	let surface_id = surface.surface_id;

	let texture = executor.create_texture_buffer(image.image.clone(), TextureBufferOptions::Texture).unwrap();

	//executor.create_render_pass(texture, surface).unwrap();

	let frame = SurfaceFrame {
		surface_id,
		transform: image.transform,
	};
	return frame;*/
	log::debug!("creating buffer");
	let width_uniform = executor.create_uniform_buffer(image.image.width).unwrap();
	#[cfg(not(feature = "quantization"))]
	core::hint::black_box(quantization);

	#[cfg(feature = "quantization")]
	let quantization_uniform = executor.create_uniform_buffer(quantization).unwrap();
	let storage_buffer = executor
		.create_storage_buffer(
			image.image.data.clone(),
			StorageBufferOptions {
				cpu_writable: false,
				gpu_writable: true,
				cpu_readable: false,
				storage: true,
			},
		)
		.unwrap();
	let width_uniform = Arc::new(width_uniform);
	#[cfg(feature = "quantization")]
	let quantization_uniform = Arc::new(quantization_uniform);
	let storage_buffer = Arc::new(storage_buffer);
	let output_buffer = executor.create_output_buffer(len, concrete!(Color), false).unwrap();
	let output_buffer = Arc::new(output_buffer);
	let readback_buffer = executor.create_output_buffer(len, concrete!(Color), true).unwrap();
	let readback_buffer = Arc::new(readback_buffer);
	log::debug!("created buffer");
	let bind_group = Bindgroup {
		#[cfg(feature = "quantization")]
		buffers: vec![width_uniform.clone(), storage_buffer.clone(), quantization_uniform.clone()],
		#[cfg(not(feature = "quantization"))]
		buffers: vec![width_uniform, storage_buffer],
	};

	let shader = gpu_executor::Shader {
		source: shader.spirv_binary.into(),
		name: "gpu::eval",
		io: shader.io,
	};
	log::debug!("loading shader");
	let shader = executor.load_shader(shader).unwrap();
	log::debug!("loaded shader");
	let pipeline = PipelineLayout {
		shader: shader.into(),
		entry_point: "eval".to_string(),
		bind_group: bind_group.into(),
		output_buffer,
	};
	log::debug!("created pipeline");

	Ok(ComputePass {
		pipeline_layout: pipeline,
		readback_buffer: Some(readback_buffer),
	})
}
/*
#[node_macro::node_fn(MapGpuNode)]
async fn map_gpu(inputs: Vec<ShaderInput<<NewExecutor as GpuExecutor>::BufferHandle>>, shader: &'any_input compilation_client::Shader) {
	use graph_craft::executor::Executor;
	let executor = NewExecutor::new().unwrap();
	for input in shader.io.inputs.iter() {
		let buffer = executor.create_storage_buffer(&self, data, options)
		let buffer = executor.create_buffer(input.size).unwrap();
		executor.write_buffer(buffer, input.data).unwrap();
	}
	todo!();
	/*
	let executor: GpuExecutor = GpuExecutor::new(Context::new().await.unwrap(), shader.into(), "gpu::eval".into()).unwrap();
	let data: Vec<_> = input.into_iter().collect();
	let result = executor.execute(Box::new(data)).unwrap();
	let result = dyn_any::downcast::<Vec<_O>>(result).unwrap();
	*result
	*/
}

pub struct MapGpuSingleImageNode<N> {
	node: N,
}

#[node_macro::node_fn(MapGpuSingleImageNode)]
fn map_gpu_single_image(input: Image<Color>, node: String) -> Image<Color> {
	use graph_craft::document::*;
	use graph_craft::ProtoNodeIdentifier;

	let identifier = ProtoNodeIdentifier { name: std::borrow::Cow::Owned(node) };

	let network = NodeNetwork {
		inputs: vec![NodeId(0)],
		disabled: vec![],
		previous_outputs: None,
		outputs: vec![NodeOutput::new(NodeId(0), 0)],
		nodes: [(
			NodeId(0),
			DocumentNode {
				name: "Image Filter".into(),
				inputs: vec![NodeInput::Network(concrete!(Color))],
				implementation: DocumentNodeImplementation::ProtoNode(identifier),
				metadata: DocumentNodeMetadata::default(),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
	};

	let value_network = ValueNode::new(network);
	let map_node = MapGpuNode::new(value_network);
	let data = map_node.eval(input.data.clone());
	Image { data, ..input }
}
*/

#[derive(Debug, Clone, Copy)]
pub struct BlendGpuImageNode<Background, B, O> {
	background: Background,
	blend_mode: B,
	opacity: O,
}

#[node_macro::node_fn(BlendGpuImageNode)]
async fn blend_gpu_image(foreground: ImageFrame<Color>, background: ImageFrame<Color>, blend_mode: BlendMode, opacity: f32) -> ImageFrame<Color> {
	let foreground_size = DVec2::new(foreground.image.width as f64, foreground.image.height as f64);
	let background_size = DVec2::new(background.image.width as f64, background.image.height as f64);
	// Transforms a point from the background image to the foreground image
	let bg_to_fg = DAffine2::from_scale(foreground_size) * foreground.transform.inverse() * background.transform * DAffine2::from_scale(1. / background_size);

	let transform_matrix: Mat2 = bg_to_fg.matrix2.as_mat2();
	let translation: Vec2 = bg_to_fg.translation.as_vec2();

	log::debug!("Executing gpu blend node!");
	let compiler = graph_craft::graphene_compiler::Compiler {};

	let network = NodeNetwork {
		inputs: vec![],
		outputs: vec![NodeOutput::new(NodeId(0), 0)],
		nodes: [DocumentNode {
			name: "BlendOp".into(),
			inputs: vec![NodeInput::Inline(InlineRust::new(
				format!(
					r#"graphene_core::raster::adjustments::BlendNode::new(
							graphene_core::value::CopiedNode::new({}),
							graphene_core::value::CopiedNode::new({}),
						).eval((
							{{
								let bg_point = Vec2::new(_global_index.x as f32, _global_index.y as f32);
								let fg_point = (*i4) * bg_point + (*i5);

								if !((fg_point.cmpge(Vec2::ZERO) & bg_point.cmpge(Vec2::ZERO)) == BVec2::new(true, true)) {{
									Color::from_rgbaf32_unchecked(0.0, 0.0, 0.0, 0.0)
								}} else {{
									i2[((fg_point.y as u32) * i3 + (fg_point.x as u32)) as usize]
								}}
							}},
							i1[(_global_index.y * i0 + _global_index.x) as usize],
						))"#,
					TaggedValue::BlendMode(blend_mode).to_primitive_string(),
					TaggedValue::F32(opacity).to_primitive_string(),
				),
				concrete![Color],
			))],
			implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::CopiedNode".into()),
			..Default::default()
		}]
		.into_iter()
		.enumerate()
		.map(|(id, node)| (NodeId(id as u64), node))
		.collect(),
		..Default::default()
	};
	log::debug!("compiling network");
	let Ok(proto_networks_result) = compiler.compile(network.clone()) else {
		log::error!("Error compiling network in 'blend_gpu_image()");
		return ImageFrame::empty();
	};
	let proto_networks = proto_networks_result.collect();
	log::debug!("compiling shader");

	let shader = compilation_client::compile(
		proto_networks,
		vec![
			concrete!(u32),
			concrete!(Color),
			concrete!(Color),
			concrete!(u32),
			concrete_with_name!(Mat2, "Mat2"),
			concrete_with_name!(Vec2, "Vec2"),
		],
		vec![concrete!(Color)],
		ShaderIO {
			inputs: vec![
				ShaderInput::UniformBuffer((), concrete!(u32)),                    // width of the output image
				ShaderInput::StorageBuffer((), concrete!(Color)),                  // background image
				ShaderInput::StorageBuffer((), concrete!(Color)),                  // foreground image
				ShaderInput::UniformBuffer((), concrete!(u32)),                    // width of the foreground image
				ShaderInput::UniformBuffer((), concrete_with_name!(Mat2, "Mat2")), // bg_to_fg.matrix2
				ShaderInput::UniformBuffer((), concrete_with_name!(Vec2, "Vec2")), // bg_to_fg.translation
				ShaderInput::OutputBuffer((), concrete!(Color)),
			],
			output: ShaderInput::OutputBuffer((), concrete!(Color)),
		},
	)
	.await
	.unwrap();
	let len = background.image.data.len();

	let executor = WgpuExecutor::new()
		.await
		.expect("Failed to create wgpu executor. Please make sure that webgpu is enabled for your browser.");
	log::debug!("creating buffer");
	let width_uniform = executor.create_uniform_buffer(background.image.width).unwrap();
	let bg_storage_buffer = executor
		.create_storage_buffer(
			background.image.data.clone(),
			StorageBufferOptions {
				cpu_writable: false,
				gpu_writable: true,
				cpu_readable: false,
				storage: true,
			},
		)
		.unwrap();
	let fg_storage_buffer = executor
		.create_storage_buffer(
			foreground.image.data.clone(),
			StorageBufferOptions {
				cpu_writable: false,
				gpu_writable: true,
				cpu_readable: false,
				storage: true,
			},
		)
		.unwrap();
	let fg_width_uniform = executor.create_uniform_buffer(foreground.image.width).unwrap();
	let transform_uniform = executor.create_uniform_buffer(transform_matrix).unwrap();
	let translation_uniform = executor.create_uniform_buffer(translation).unwrap();
	let width_uniform = Arc::new(width_uniform);
	let bg_storage_buffer = Arc::new(bg_storage_buffer);
	let fg_storage_buffer = Arc::new(fg_storage_buffer);
	let fg_width_uniform = Arc::new(fg_width_uniform);
	let transform_uniform = Arc::new(transform_uniform);
	let translation_uniform = Arc::new(translation_uniform);
	let output_buffer = executor.create_output_buffer(len, concrete!(Color), false).unwrap();
	let output_buffer = Arc::new(output_buffer);
	let readback_buffer = executor.create_output_buffer(len, concrete!(Color), true).unwrap();
	let readback_buffer = Arc::new(readback_buffer);
	log::debug!("created buffer");
	let bind_group = Bindgroup {
		buffers: vec![
			width_uniform.clone(),
			bg_storage_buffer.clone(),
			fg_storage_buffer.clone(),
			fg_width_uniform.clone(),
			transform_uniform.clone(),
			translation_uniform.clone(),
		],
	};

	let shader = gpu_executor::Shader {
		source: shader.spirv_binary.into(),
		name: "gpu::eval",
		io: shader.io,
	};
	log::debug!("loading shader");
	log::debug!("shader: {:?}", shader.source);
	let shader = executor.load_shader(shader).unwrap();
	log::debug!("loaded shader");
	let pipeline = PipelineLayout {
		shader: shader.into(),
		entry_point: "eval".to_string(),
		bind_group: bind_group.into(),
		output_buffer: output_buffer.clone(),
	};
	log::debug!("created pipeline");
	let compute_pass = executor
		.create_compute_pass(&pipeline, Some(readback_buffer.clone()), ComputePassDimensions::XY(background.image.width, background.image.height))
		.unwrap();
	executor.execute_compute_pipeline(compute_pass).unwrap();
	log::debug!("executed pipeline");
	log::debug!("reading buffer");
	let result = executor.read_output_buffer(readback_buffer).await.unwrap();
	let colors = bytemuck::pod_collect_to_vec::<u8, Color>(result.as_slice());

	ImageFrame {
		image: Image {
			data: colors,
			width: background.image.width,
			height: background.image.height,
			..Default::default()
		},
		transform: background.transform,
		alpha_blending: background.alpha_blending,
	}
}
