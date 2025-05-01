//! This has all been copied out of node_registry.rs to avoid leaving many lines of commented out code in that file. It's left here instead for future reference.

// (
// 	ProtoNodeIdentifier::new("graphene_core::raster::CurvesNode"),
// 	|args| {
// 		use graphene_core::raster::curve::Curve;
// 		use graphene_core::raster::GenerateCurvesNode;
// 		let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0].clone());
// 		Box::pin(async move {
// 			let curve = ClonedNode::new(curve.eval(()).await);

// 			let generate_curves_node = GenerateCurvesNode::new(curve, ClonedNode::new(0_f32));
// 			let map_image_frame_node = graphene_std::raster::MapImageNode::new(ValueNode::new(generate_curves_node.eval(())));
// 			let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
// 			let any: DynAnyNode<ImageFrameTable<Luma>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
// 			any.into_type_erased()
// 		})
// 	},
// 	NodeIOTypes::new(concrete!(ImageFrameTable<Luma>), concrete!(ImageFrameTable<Luma>), vec![fn_type!(graphene_core::raster::curve::Curve)]),
// ),
// TODO: Use channel split and merge for this instead of using LuminanceMut for the whole color.
// (
// 	ProtoNodeIdentifier::new("graphene_core::raster::CurvesNode"),
// 	|args| {
// 		use graphene_core::raster::curve::Curve;
// 		use graphene_core::raster::GenerateCurvesNode;
// 		let curve: DowncastBothNode<(), Curve> = DowncastBothNode::new(args[0].clone());
// 		Box::pin(async move {
// 			let curve = ValueNode::new(ClonedNode::new(curve.eval(()).await));

// 			let generate_curves_node = GenerateCurvesNode::new(FutureWrapperNode::new(curve), FutureWrapperNode::new(ClonedNode::new(0_f32)));
// 			let map_image_frame_node = graphene_std::raster::MapImageNode::new(FutureWrapperNode::new(ValueNode::new(generate_curves_node.eval(()))));
// 			let map_image_frame_node = FutureWrapperNode::new(map_image_frame_node);
// 			let any: DynAnyNode<ImageFrameTable<Color>, _, _> = graphene_std::any::DynAnyNode::new(map_image_frame_node);
// 			any.into_type_erased()
// 		})
// 	},
// 	NodeIOTypes::new(
// 		concrete!(ImageFrameTable<Color>),
// 		concrete!(ImageFrameTable<Color>),
// 		vec![fn_type!(graphene_core::raster::curve::Curve)],
// 	),
// ),
// (
// 	ProtoNodeIdentifier::new("graphene_std::raster::ImaginateNode"),
// 	|args: Vec<graph_craft::proto::SharedNodeContainer>| {
// 		Box::pin(async move {
// 			use graphene_std::raster::ImaginateNode;
// 			macro_rules! instantiate_imaginate_node {
// 						($($i:expr,)*) => { ImaginateNode::new($(graphene_std::any::input_node(args[$i].clone()),)* ) };
// 					}
// 			let node: ImaginateNode<Color, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _> = instantiate_imaginate_node!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,);
// 			let any = graphene_std::any::DynAnyNode::new(node);
// 			any.into_type_erased()
// 		})
// 	},
// 	NodeIOTypes::new(
// 		concrete!(ImageFrameTable<Color>),
// 		concrete!(ImageFrameTable<Color>),
// 		vec![
// 			fn_type!(&WasmEditorApi),
// 			fn_type!(ImaginateController),
// 			fn_type!(f64),
// 			fn_type!(Option<DVec2>),
// 			fn_type!(u32),
// 			fn_type!(ImaginateSamplingMethod),
// 			fn_type!(f64),
// 			fn_type!(String),
// 			fn_type!(String),
// 			fn_type!(bool),
// 			fn_type!(f64),
// 			fn_type!(bool),
// 			fn_type!(f64),
// 			fn_type!(ImaginateMaskStartingFill),
// 			fn_type!(bool),
// 			fn_type!(bool),
// 			fn_type!(u64),
// 		],
// 	),
// ),
