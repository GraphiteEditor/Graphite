use glam::{DAffine2, DVec2, Mat2, Vec2};
use gpu_executor::{ComputePassDimensions, StorageBufferOptions};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;
use graph_craft::proto::*;
use graphene_core::raster::BlendMode;
use graphene_core::raster::image::{Image, ImageFrameTable};
use graphene_core::*;
use std::sync::Arc;
use wgpu_executor::{Bindgroup, PipelineLayout, Shader, ShaderIO, ShaderInput, WgpuExecutor};

// TODO: Move to graph-craft
#[node_macro::node(category("Debug: GPU"))]
async fn compile_gpu<'a: 'n>(_: impl Ctx, node: &'a DocumentNode, typing_context: TypingContext, io: ShaderIO) -> Result<compilation_client::Shader, String> {
	let mut typing_context = typing_context;
	let compiler = graph_craft::graphene_compiler::Compiler {};
	let DocumentNodeImplementation::Network(ref network) = node.implementation else { panic!() };
	let proto_networks: Result<Vec<_>, _> = compiler.compile(network.clone()).collect();
	let proto_networks = proto_networks?;

	for network in proto_networks.iter() {
		typing_context.update(network).expect("Failed to type check network");
	}
	// TODO: do a proper union
	let input_types = proto_networks[0]
		.inputs
		.iter()
		.map(|id| typing_context.type_of(*id).unwrap())
		.map(|node_io| node_io.return_value.clone())
		.collect();
	let output_types = proto_networks.iter().map(|network| typing_context.type_of(network.output).unwrap().return_value.clone()).collect();

	Ok(compilation_client::compile(proto_networks, input_types, output_types, io).await.unwrap())
}

#[node_macro::node(category("Debug: GPU"))]
async fn blend_gpu_image(_: impl Ctx, foreground: ImageFrameTable<Color>, background: ImageFrameTable<Color>, blend_mode: BlendMode, opacity: f64) -> ImageFrameTable<Color> {
	let mut result_table = ImageFrameTable::default();

	for (foreground_instance, mut background_instance) in foreground.instance_iter().zip(background.instance_iter()) {
		let foreground_transform = foreground_instance.transform;
		let background_transform = background_instance.transform;

		let foreground = foreground_instance.instance;
		let background = background_instance.instance;

		let foreground_size = DVec2::new(foreground.width as f64, foreground.height as f64);
		let background_size = DVec2::new(background.width as f64, background.height as f64);

		// Transforms a point from the background image to the foreground image
		let bg_to_fg = DAffine2::from_scale(foreground_size) * foreground_transform.inverse() * background_transform * DAffine2::from_scale(1. / background_size);

		let transform_matrix: Mat2 = bg_to_fg.matrix2.as_mat2();
		let translation: Vec2 = bg_to_fg.translation.as_vec2();

		log::debug!("Executing gpu blend node!");
		let compiler = graph_craft::graphene_compiler::Compiler {};

		let network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(0), 0)],
			nodes: [DocumentNode {
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
									Color::from_rgbaf32_unchecked(0., 0., 0., 0.)
								}} else {{
									i2[((fg_point.y as u32) * i3 + (fg_point.x as u32)) as usize]
								}}
							}},
							i1[(_global_index.y * i0 + _global_index.x) as usize],
						))"#,
						TaggedValue::BlendMode(blend_mode).to_primitive_string(),
						TaggedValue::F64(opacity).to_primitive_string(),
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
		let proto_networks: Result<Vec<_>, _> = compiler.compile(network.clone()).collect();
		let Ok(proto_networks_result) = proto_networks else {
			log::error!("Error compiling network in 'blend_gpu_image()");
			return ImageFrameTable::default();
		};
		let proto_networks = proto_networks_result;
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
		let len = background.data.len();

		let executor = WgpuExecutor::new()
			.await
			.expect("Failed to create wgpu executor. Please make sure that webgpu is enabled for your browser.");
		log::debug!("creating buffer");
		let width_uniform = executor.create_uniform_buffer(background.width).unwrap();
		let bg_storage_buffer = executor
			.create_storage_buffer(
				background.data.clone(),
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
				foreground.data.clone(),
				StorageBufferOptions {
					cpu_writable: false,
					gpu_writable: true,
					cpu_readable: false,
					storage: true,
				},
			)
			.unwrap();
		let fg_width_uniform = executor.create_uniform_buffer(foreground.width).unwrap();
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

		let shader = Shader {
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
			.create_compute_pass(&pipeline, Some(readback_buffer.clone()), ComputePassDimensions::XY(background.width, background.height))
			.unwrap();
		executor.execute_compute_pipeline(compute_pass).unwrap();
		log::debug!("executed pipeline");
		log::debug!("reading buffer");
		let result = executor.read_output_buffer(readback_buffer).await.unwrap();
		let colors = bytemuck::pod_collect_to_vec::<u8, Color>(result.as_slice());

		let created_image = Image {
			data: colors,
			width: background.width,
			height: background.height,
			..Default::default()
		};

		background_instance.instance = created_image;
		background_instance.source_node_id = None;
		result_table.push(background_instance);
	}

	result_table
}

// struct ComputePass {
// 	pipeline_layout: PipelineLayout,
// 	readback_buffer: Option<Arc<WgpuShaderInput>>,
// }

// impl Clone for ComputePass {
// 	fn clone(&self) -> Self {
// 		Self {
// 			pipeline_layout: self.pipeline_layout.clone(),
// 			readback_buffer: self.readback_buffer.clone(),
// 		}
// 	}
// }

// pub struct MapGpuNode<Node, EditorApi> {
// 	node: Node,
// 	editor_api: EditorApi,
// 	cache: Mutex<HashMap<String, ComputePass>>,
// }

// #[node_macro::old_node_impl(MapGpuNode)]
// async fn map_gpu<'a: 'input>(image: ImageFrameTable<Color>, node: DocumentNode, editor_api: &'a graphene_core::application_io::EditorApi<WasmApplicationIo>) -> ImageFrameTable<Color> {
// 	let image_frame_table = &image;
// 	let image = image.instance_ref_iter().next().unwrap().instance;

// 	log::debug!("Executing gpu node");
// 	let executor = &editor_api.application_io.as_ref().and_then(|io| io.gpu_executor()).unwrap();

// 	#[cfg(feature = "image-compare")]
// 	let img: image::DynamicImage = image::Rgba32FImage::from_raw(image.width, image.height, bytemuck::cast_vec(image.data.clone())).unwrap().into();

// 	// TODO: The cache should be based on the network topology not the node name
// 	let compute_pass_descriptor = if self.cache.lock().as_ref().unwrap().contains_key("placeholder") {
// 		self.cache.lock().as_ref().unwrap().get("placeholder").unwrap().clone()
// 	} else {
// 		let name = "placeholder".to_string();
// 		let Ok(compute_pass_descriptor) = create_compute_pass_descriptor(node, image_frame_table, executor).await else {
// 			log::error!("Error creating compute pass descriptor in 'map_gpu()");
// 			return ImageFrameTable::default();
// 		};
// 		self.cache.lock().as_mut().unwrap().insert(name, compute_pass_descriptor.clone());
// 		log::error!("created compute pass");
// 		compute_pass_descriptor
// 	};

// 	let compute_pass = executor
// 		.create_compute_pass(
// 			&compute_pass_descriptor.pipeline_layout,
// 			compute_pass_descriptor.readback_buffer.clone(),
// 			ComputePassDimensions::XY(image.width / 12 + 1, image.height / 8 + 1),
// 		)
// 		.unwrap();
// 	executor.execute_compute_pipeline(compute_pass).unwrap();
// 	log::debug!("executed pipeline");
// 	log::debug!("reading buffer");
// 	let result = executor.read_output_buffer(compute_pass_descriptor.readback_buffer.clone().unwrap()).await.unwrap();
// 	let colors = bytemuck::pod_collect_to_vec::<u8, Color>(result.as_slice());
// 	log::debug!("first color: {:?}", colors[0]);

// 	#[cfg(feature = "image-compare")]
// 	let img2: image::DynamicImage = image::Rgba32FImage::from_raw(image.width, image.height, bytemuck::cast_vec(colors.clone())).unwrap().into();
// 	#[cfg(feature = "image-compare")]
// 	let score = image_compare::rgb_hybrid_compare(&img.into_rgb8(), &img2.into_rgb8()).unwrap();
// 	#[cfg(feature = "image-compare")]
// 	log::debug!("score: {:?}", score.score);

// 	let new_image = Image {
// 		data: colors,
// 		width: image.width,
// 		height: image.height,
// 		..Default::default()
// 	};
// 	let mut result = ImageFrameTable::new(new_image);
// 	*result.transform_mut() = image_frame_table.transform();
// 	*result.instance_mut_iter().next().unwrap().alpha_blending = *image_frame_table.instance_ref_iter().next().unwrap().alpha_blending;

// 	result
// }

// impl<Node, EditorApi> MapGpuNode<Node, EditorApi> {
// 	pub fn new(node: Node, editor_api: EditorApi) -> Self {
// 		Self {
// 			node,
// 			editor_api,
// 			cache: Mutex::new(HashMap::new()),
// 		}
// 	}
// }

// async fn create_compute_pass_descriptor<T: Clone + Pixel + StaticTypeSized>(node: DocumentNode, image: &ImageFrameTable<T>, executor: &&WgpuExecutor) -> Result<ComputePass, String>
// where
// 	GraphicElement: From<Image<T>>,
// 	T::Static: Pixel,
// {
// 	let image = image.instance_ref_iter().next().unwrap().instance;

// 	let compiler = graph_craft::graphene_compiler::Compiler {};
// 	let inner_network = NodeNetwork::value_network(node);

// 	log::debug!("inner_network: {inner_network:?}");
// 	let network = NodeNetwork {
// 		exports: vec![NodeInput::node(NodeId(2), 0)],
// 		nodes: [
// 			DocumentNode {
// 				inputs: vec![NodeInput::Inline(InlineRust::new("i1[(_global_index.y * i0 + _global_index.x) as usize]".into(), concrete![Color]))],
// 				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::CopiedNode".into()),
// 				..Default::default()
// 			},
// 			DocumentNode {
// 				inputs: vec![NodeInput::network(concrete!(u32), 0)],
// 				implementation: DocumentNodeImplementation::ProtoNode("graphene_core::ops::IdentityNode".into()),
// 				..Default::default()
// 			},
// 			// DocumentNode {
// 			// 	name: "Index".into(),
// 			// 	// inputs: vec![NodeInput::Network(concrete!(UVec3))],
// 			// 	inputs: vec![NodeInput::Inline(InlineRust::new("i1.x as usize".into(), concrete![u32]))],
// 			// 	implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::CopiedNode".into()),
// 			// 	..Default::default()
// 			// },
// 			// DocumentNode {
// 			// 	name: "Get Node".into(),
// 			// 	inputs: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(0), 0)],
// 			// 	implementation: DocumentNodeImplementation::ProtoNode("graphene_core::storage::GetNode".into()),
// 			// 	..Default::default()
// 			// },
// 			DocumentNode {
// 				inputs: vec![NodeInput::node(NodeId(0), 0)],
// 				implementation: DocumentNodeImplementation::Network(inner_network),
// 				..Default::default()
// 			},
// 			// DocumentNode {
// 			// 	name: "Save Node".into(),
// 			// 	inputs: vec![
// 			// 		NodeInput::node(NodeId(5), 0),
// 			// 		NodeInput::Inline(InlineRust::new(
// 			// 			"|x| o0[(_global_index.y * i1 + _global_index.x) as usize] = x".into(),
// 			// 			// "|x|()".into(),
// 			// 			Type::Fn(Box::new(concrete!(PackedPixel)), Box::new(concrete!(()))),
// 			// 		)),
// 			// 	],
// 			// 	implementation: DocumentNodeImplementation::ProtoNode("graphene_core::generic::FnMutNode".into()),
// 			// 	..Default::default()
// 			// },
// 		]
// 		.into_iter()
// 		.enumerate()
// 		.map(|(id, node)| (NodeId(id as u64), node))
// 		.collect(),
// 		..Default::default()
// 	};
// 	log::debug!("compiling network");
// 	let proto_networks: Result<Vec<_>, _> = compiler.compile(network.clone()).collect();
// 	log::debug!("compiling shader");
// 	let shader = compilation_client::compile(
// 		proto_networks?,
// 		vec![concrete!(u32), concrete!(Color)],
// 		vec![concrete!(Color)],
// 		ShaderIO {
// 			inputs: vec![
// 				ShaderInput::UniformBuffer((), concrete!(u32)),
// 				ShaderInput::StorageBuffer((), concrete!(Color)),
// 				ShaderInput::OutputBuffer((), concrete!(Color)),
// 			],
// 			output: ShaderInput::OutputBuffer((), concrete!(Color)),
// 		},
// 	)
// 	.await
// 	.unwrap();

// 	let len: usize = image.data.len();

// 	let storage_buffer = executor
// 		.create_storage_buffer(
// 			image.data.clone(),
// 			StorageBufferOptions {
// 				cpu_writable: false,
// 				gpu_writable: true,
// 				cpu_readable: false,
// 				storage: true,
// 			},
// 		)
// 		.unwrap();

// 	// let canvas = editor_api.application_io.create_surface();

// 	// let surface = unsafe { executor.create_surface(canvas) }.unwrap();
// 	// let surface_id = surface.surface_id;

// 	// let texture = executor.create_texture_buffer(image.clone(), TextureBufferOptions::Texture).unwrap();

// 	// // executor.create_render_pass(texture, surface).unwrap();

// 	// let frame = SurfaceFrame {
// 	// 	surface_id,
// 	// 	transform: image.transform,
// 	// };
// 	// return frame;

// 	log::debug!("creating buffer");
// 	let width_uniform = executor.create_uniform_buffer(image.width).unwrap();

// 	let storage_buffer = Arc::new(storage_buffer);
// 	let output_buffer = executor.create_output_buffer(len, concrete!(Color), false).unwrap();
// 	let output_buffer = Arc::new(output_buffer);
// 	let readback_buffer = executor.create_output_buffer(len, concrete!(Color), true).unwrap();
// 	let readback_buffer = Arc::new(readback_buffer);
// 	log::debug!("created buffer");
// 	let bind_group = Bindgroup {
// 		buffers: vec![width_uniform.into(), storage_buffer],
// 	};

// 	let shader = Shader {
// 		source: shader.spirv_binary.into(),
// 		name: "gpu::eval",
// 		io: shader.io,
// 	};
// 	log::debug!("loading shader");
// 	let shader = executor.load_shader(shader).unwrap();
// 	log::debug!("loaded shader");
// 	let pipeline = PipelineLayout {
// 		shader: shader.into(),
// 		entry_point: "eval".to_string(),
// 		bind_group: bind_group.into(),
// 		output_buffer,
// 	};
// 	log::debug!("created pipeline");

// 	Ok(ComputePass {
// 		pipeline_layout: pipeline,
// 		readback_buffer: Some(readback_buffer),
// 	})
// }
