use glam::{DAffine2, DVec2, IVec2};
use graphene_std::registry::{ConstructionError, NodeIOTypes, RegistryEntry, SourceHandle};
use graphene_std::{Context, ProtoNodeIdentifier, concrete};
use node_registry_macros::convert_node;
use std::collections::HashMap;

fn node_registry() -> HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>> {
	let mut node_types: Vec<(ProtoNodeIdentifier, RegistryEntry)> = vec![
		// =============
		// CONVERT NODES
		// =============
		convert_node!(from: DVec2, to: DVec2),
		convert_node!(from: String, to: String),
		convert_node!(from: bool, to: String),
		convert_node!(from: DVec2, to: String),
		convert_node!(from: IVec2, to: String),
		convert_node!(from: DAffine2, to: String),
	];
	// The transform's value-typed rows, served by `transform_value` under the
	// leveled transform's identifier.
	node_types.extend(
		graphene_std::transform_nodes::transform_nodes::transform_value_entries()
			.into_iter()
			.map(|entry| (graphene_std::transform_nodes::transform_nodes::transform::IDENTIFIER.clone(), entry)),
	);
	// The graphic-lane fill and stroke rows, served under their identifiers.
	node_types.extend(
		graphene_std::vector::fill_graphic_leveled_entries()
			.into_iter()
			.map(|entry| (graphene_std::vector::fill::IDENTIFIER.clone(), entry)),
	);
	node_types.extend(
		graphene_std::vector::stroke_graphic_leveled_entries()
			.into_iter()
			.map(|entry| (graphene_std::vector::stroke::IDENTIFIER.clone(), entry)),
	);
	// The boolean operation's plain vector rows, served under its identifier.
	node_types.extend(
		graphene_std::path_bool_nodes::boolean_operation_vector_entries()
			.into_iter()
			.map(|entry| (graphene_std::path_bool_nodes::boolean_operation::IDENTIFIER.clone(), entry)),
	);
	// The path flattening's plain vector rows, served under its identifier.
	node_types.extend(
		graphene_std::vector::flatten_path_vector_entries()
			.into_iter()
			.map(|entry| (graphene_std::vector::flatten_path::IDENTIFIER.clone(), entry)),
	);
	// The solidify's plain vector rows, served under its identifier.
	node_types.extend(
		graphene_std::vector::solidify_stroke_vector_entries()
			.into_iter()
			.map(|entry| (graphene_std::vector::solidify_stroke::IDENTIFIER.clone(), entry)),
	);
	// The color assignment's graphic-lane rows, served under its identifier.
	node_types.extend(
		graphene_std::vector::assign_colors_graphic_entries()
			.into_iter()
			.map(|entry| (graphene_std::vector::assign_colors::IDENTIFIER.clone(), entry)),
	);
	// The mirror's plain vector rows, served under its identifier.
	node_types.extend(
		graphene_std::graphic::mirror_vector_entries()
			.into_iter()
			.map(|entry| (graphene_std::graphic::mirror::IDENTIFIER.clone(), entry)),
	);
	// The gradient over a graphic level's color leaves, served under the colors to gradient identifier.
	node_types.extend(
		graphene_std::graphic::colors_to_gradient_graphic_entries()
			.into_iter()
			.map(|entry| (graphene_std::graphic::colors_to_gradient::IDENTIFIER.clone(), entry)),
	);
	// The morph's plain vector rows, served under its identifier.
	node_types.extend(
		graphene_std::vector::morph_vector_entries()
			.into_iter()
			.map(|entry| (graphene_std::vector::morph::IDENTIFIER.clone(), entry)),
	);
	// Element-wise coercion into `Graphic` for single-typed leveled inputs,
	// served by the hidden elementwise rows.
	node_types.extend(
		graphene_std::graphic::to_graphic_element_entries()
			.into_iter()
			.map(|entry| (ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<Graphic>"), entry)),
	);
	// The typed-level collapse rows of To Graphic, served under its identifier.
	node_types.extend(
		graphene_std::graphic::to_graphic_typed_entries()
			.into_iter()
			.map(|entry| (graphene_std::graphic::to_graphic::IDENTIFIER.clone(), entry)),
	);
	// The unit row of To Graphic: an unconnected content input renders as nothing.
	node_types.extend(
		graphene_std::graphic::to_graphic_unit_entries()
			.into_iter()
			.map(|entry| (graphene_std::graphic::to_graphic::IDENTIFIER.clone(), entry)),
	);
	// =============
	// CONVERT NODES
	// =============
	node_types.extend(
		[
			convert_node!(from: f32, to: numbers),
			convert_node!(from: f64, to: numbers),
			convert_node!(from: i8, to: numbers),
			convert_node!(from: u8, to: numbers),
			convert_node!(from: u16, to: numbers),
			convert_node!(from: i16, to: numbers),
			convert_node!(from: i32, to: numbers),
			convert_node!(from: u32, to: numbers),
			convert_node!(from: i64, to: numbers),
			convert_node!(from: u64, to: numbers),
			convert_node!(from: i128, to: numbers),
			convert_node!(from: u128, to: numbers),
			convert_node!(from: isize, to: numbers),
			convert_node!(from: usize, to: numbers),
			convert_node!(from: numbers, to: DVec2),
			convert_node!(from: numbers, to: String),
		]
		.into_iter()
		.flatten(),
	);

	let mut map: HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>> = HashMap::new();
	let insert = |map: &mut HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>>, id: ProtoNodeIdentifier, entry: RegistryEntry| {
		let rows = map.entry(id).or_default();
		if !rows.iter().any(|row| row.io == entry.io) {
			rows.push(entry);
		}
	};

	for (id, entries) in graphene_std::registry::NODE_REGISTRY.lock().unwrap().iter() {
		for entry in entries {
			insert(&mut map, id.clone(), entry.clone());
		}
	}

	for (id, entry) in node_types.into_iter() {
		// TODO: this is a hack to remove the newline from the node new_name
		// This occurs for the ChannelMixerNode presumably because of the long name.
		// This might be caused by the stringify! macro
		let mut new_name = id.as_str().replace('\n', " ");

		// Remove struct generics for all nodes except for the IntoNode and ConvertNode
		if !(new_name.contains("IntoNode") || new_name.contains("ConvertNode"))
			&& let Some((path, _generics)) = new_name.split_once("<")
		{
			new_name = path.to_string();
		}

		insert(&mut map, ProtoNodeIdentifier::with_owned_string(new_name), entry);
	}

	map
}

// TODO: Replace with `core::cell::LazyCell` (<https://doc.rust-lang.org/core/cell/struct.LazyCell.html>) or similar
pub static NODE_REGISTRY: once_cell::sync::Lazy<HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>>> = once_cell::sync::Lazy::new(node_registry);

mod node_registry_macros {
	macro_rules! convert_node {
		(from: $from:ty, to: numbers) => {{
			let x: Vec<(ProtoNodeIdentifier, RegistryEntry)> = vec![
				convert_node!(from: $from, to: f32),
				convert_node!(from: $from, to: f64),
				convert_node!(from: $from, to: i8),
				convert_node!(from: $from, to: u8),
				convert_node!(from: $from, to: u16),
				convert_node!(from: $from, to: i16),
				convert_node!(from: $from, to: i32),
				convert_node!(from: $from, to: u32),
				convert_node!(from: $from, to: i64),
				convert_node!(from: $from, to: u64),
				convert_node!(from: $from, to: i128),
				convert_node!(from: $from, to: u128),
				convert_node!(from: $from, to: isize),
				convert_node!(from: $from, to: usize),
			];
			x
		}};
		(from: numbers, to: $to:ty) => {{
			let x: Vec<(ProtoNodeIdentifier, RegistryEntry)> = vec![
				convert_node!(from: f32, to: $to),
				convert_node!(from: f64, to: $to),
				convert_node!(from: i8, to: $to),
				convert_node!(from: u8, to: $to),
				convert_node!(from: u16, to: $to),
				convert_node!(from: i16, to: $to),
				convert_node!(from: i32, to: $to),
				convert_node!(from: u32, to: $to),
				convert_node!(from: i64, to: $to),
				convert_node!(from: u64, to: $to),
				convert_node!(from: i128, to: $to),
				convert_node!(from: u128, to: $to),
				convert_node!(from: isize, to: $to),
				convert_node!(from: usize, to: $to),
			];
			x
		}};
		(from: $from:ty, to: $to:ty) => {
			convert_node!(from: $from, to: $to, converter: ())
		};
		(from: $from:ty, to: $to:ty, converter: $convert:ty, async) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ConvertNode<", stringify!($to), ">"]),
				RegistryEntry {
					layout_meta: Some(core_types::record::LayoutMeta::retype(core_types::record::element_write::<$to>())),
					io: NodeIOTypes::new(
						concrete!(Context),
						core_types::registry::record_type::<$to>(),
						vec![
							core_types::registry::record_source_type::<$from>(),
							core_types::registry::record_source_type::<$convert>(),
							core_types::registry::record_source_type::<RuntimeHandle>(),
							core_types::registry::record_source_type::<SourceId>(),
						],
					),
					constructor: |inputs| {
						if inputs.len() != 4 {
							return Err(ConstructionError::Arity { expected: 4, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let mut claim = || {
							let handle = inputs.next().unwrap();
							let layout = handle.layout().clone();
							Ok::<_, ConstructionError>((handle, layout))
						};
						let (value, value_layout) = claim()?;
						let (converter, converter_layout) = claim()?;
						let (runtime, runtime_layout) = claim()?;
						let (source, source_layout) = claim()?;
						let node = graphene_std::ops::ConvertAsyncNode::<$to, _, _, _, _, $from, $convert>::new(
							value.downcast_record::<$from>()?,
							converter.downcast_record::<$convert>()?,
							runtime.downcast_record::<RuntimeHandle>()?,
							source.downcast_record::<SourceId>()?,
							&value_layout,
							&converter_layout,
							&runtime_layout,
							&source_layout,
						);
						Ok(SourceHandle::new_record::<$to>(std::sync::Arc::new(node) as std::sync::Arc<core_types::registry::ErasedRecordNode>))
					},
				},
			)
		};
		(from: $from:ty, to: $to:ty, converter: $convert:ty) => {
			(
				ProtoNodeIdentifier::new(concat!["graphene_core::ops::ConvertNode<", stringify!($to), ">"]),
				RegistryEntry {
					layout_meta: Some(core_types::record::LayoutMeta::retype(core_types::record::element_write::<$to>())),
					io: NodeIOTypes::new(
						concrete!(Context),
						core_types::registry::record_type::<$to>(),
						vec![core_types::registry::record_source_type::<$from>(), core_types::registry::record_source_type::<$convert>()],
					),
					constructor: |inputs| {
						if inputs.len() != 2 {
							return Err(ConstructionError::Arity { expected: 2, got: inputs.len() });
						}
						let mut inputs = inputs.into_iter();
						let value = inputs.next().unwrap();
						let value_layout = value.layout().clone();
						let converter = inputs.next().unwrap();
						let converter_layout = converter.layout().clone();
						let node = graphene_std::ops::ConvertNode::<$to, _, _, $from, $convert>::new(
							value.downcast_record::<$from>()?,
							converter.downcast_record::<$convert>()?,
							&value_layout,
							&converter_layout,
						);
						Ok(SourceHandle::new_record::<$to>(std::sync::Arc::new(node) as std::sync::Arc<core_types::registry::ErasedRecordNode>))
					},
				},
			)
		};
	}

	pub(crate) use convert_node;
}

#[cfg(test)]
mod tests {
	use super::*;
	use graphene_std::Type;

	fn is_record_edge(ty: &Type) -> bool {
		match ty {
			Type::Fn(_, output) => matches!(&**output, Type::Record(_)),
			_ => false,
		}
	}

	/// One input kind: every row consumes and produces records. A plain
	/// io type here would need a bridge adapter, and those are gone.
	#[test]
	fn every_registry_row_is_record_typed() {
		let mut plain: Vec<String> = Vec::new();
		for (id, entries) in NODE_REGISTRY.iter() {
			for entry in entries {
				let plain_output = !matches!(entry.io.return_value, Type::Record(_));
				let plain_inputs: Vec<usize> = entry.io.inputs.iter().enumerate().filter(|(_, ty)| !is_record_edge(ty)).map(|(index, _)| index).collect();
				if plain_output || !plain_inputs.is_empty() {
					plain.push(format!("{} plain_output={plain_output} plain_inputs={plain_inputs:?}", id.as_str()));
				}
			}
		}
		plain.sort();
		assert!(plain.is_empty(), "plain io remains in the registry:\n{}", plain.join("\n"));
	}
}
