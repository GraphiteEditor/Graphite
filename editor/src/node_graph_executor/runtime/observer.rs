use super::{context_hash, evaluated_output};
use crate::messages::portfolio::document::data_panel::shareable_inspect_output;
use glam::DVec2;
use graph_craft::document::{NodeId, NodeNetwork};
use graphene_std::{Context, ContextFeatures, ContextOverrides, OwnedContextImpl};
use interpreted_executor::dynamic_executor::{BorrowTree, DynamicExecutor};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

pub type ObservedValue = Arc<dyn std::any::Any + Send + Sync + 'static>;

/// A connector of a document node whose value the editor reads back after each evaluation.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Observation {
	pub node_path: Vec<NodeId>,
	pub connector: ObservedConnector,
	/// Replaces parts of the context the value is evaluated with.
	pub overrides: ContextOverrides,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObservedConnector {
	Input(usize),
	Output,
}

/// The value an observation read, with the context it was evaluated with.
#[derive(Clone, Debug, Default)]
pub struct InspectResult {
	pub data: Option<ObservedValue>,
	pub node_path: Vec<NodeId>,
	pub context: InspectedContext,
}

/// The features of the context an inspected node was evaluated with, after the overrides replaced some of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectedContext {
	pub features: ContextOverrides,
	pub positions: Vec<DVec2>,
	pub variables: usize,
	pub overrides: ContextOverrides,
	pub dependencies: ContextFeatures,
}

// This is very ugly but is required to be inside a message
impl PartialEq for InspectResult {
	fn eq(&self, other: &Self) -> bool {
		self.node_path == other.node_path
	}
}

/// Evaluates each observed connector again from the context its node was last evaluated with.
pub struct Observer<Key> {
	observations: HashMap<Key, ObservationState>,
}

struct ObservationState {
	observation: Observation,
	last_evaluation: Option<Evaluation>,
}

struct Evaluation {
	source: NodeId,
	context_hash: u64,
	value: Option<ObservedValue>,
}

impl<Key> Default for Observer<Key> {
	fn default() -> Self {
		Self { observations: HashMap::new() }
	}
}

impl<Key: Clone + Eq + Hash> Observer<Key> {
	/// Replaces every observation, keeping the last evaluation of those that stayed the same.
	pub fn replace_all(&mut self, observations: HashMap<Key, Observation>) {
		let mut previous = std::mem::take(&mut self.observations);
		self.observations = observations
			.into_iter()
			.map(|(key, observation)| {
				let state = ObservationState::new(observation, previous.remove(&key));
				(key, state)
			})
			.collect();
	}

	/// Starts, changes, or with `None` stops the observation under this key.
	pub fn set(&mut self, key: Key, observation: Option<Observation>) {
		let previous = self.observations.remove(&key);
		let Some(observation) = observation else { return };

		self.observations.insert(key, ObservationState::new(observation, previous));
	}

	/// Evaluates the observations whose source or context changed, giving back the latest result of each.
	pub fn evaluate(&mut self, executor: &DynamicExecutor, network: Option<&NodeNetwork>) -> HashMap<Key, InspectResult> {
		let tree = executor.tree();

		self.observations.iter_mut().map(|(key, state)| (key.clone(), state.evaluate(tree, network))).collect()
	}
}

impl ObservationState {
	fn new(observation: Observation, previous: Option<ObservationState>) -> Self {
		let last_evaluation = previous.filter(|previous| previous.observation == observation).and_then(|previous| previous.last_evaluation);
		Self { observation, last_evaluation }
	}

	fn evaluate(&mut self, tree: &BorrowTree, network: Option<&NodeNetwork>) -> InspectResult {
		let Observation { node_path, connector, overrides } = &self.observation;

		let source = match connector {
			ObservedConnector::Input(index) => tree.input_node(node_path, *index).ok(),
			ObservedConnector::Output => network.and_then(|network| tree.output_node(network, node_path)),
		};
		let context = source.and_then(|source| tree.last_context(source).ok().flatten()).map(|context| with_overrides(context, overrides));

		let introspected_data = source
			.zip(context.clone())
			.and_then(|(source, context)| evaluate_value(&mut self.last_evaluation, tree, source, context));

		let features = context.as_ref().and_then(|context| context.as_deref()).map(context_features).unwrap_or_default();
		let context = InspectedContext {
			overrides: overrides.clone(),
			dependencies: source.map(|source| tree.context_features(source)).unwrap_or_default(),
			..features
		};

		InspectResult {
			data: introspected_data,
			node_path: node_path.clone(),
			context,
		}
	}
}

/// Evaluates the node with the context, reusing the last result if neither changed.
fn evaluate_value(last_evaluation: &mut Option<Evaluation>, tree: &BorrowTree, source: NodeId, context: Context<'static>) -> Option<ObservedValue> {
	let context_hash = context_hash(&context);
	if let Some(evaluation) = last_evaluation
		&& (evaluation.source, evaluation.context_hash) == (source, context_hash)
	{
		return evaluation.value.clone();
	}

	let output = evaluated_output(tree.evaluate(source, &context).ok()?)?;
	// Output that can't be shared is remembered too, so it isn't evaluated again for nothing
	let value = shareable_inspect_output(output);
	*last_evaluation = Some(Evaluation {
		source,
		context_hash,
		value: value.clone(),
	});
	value
}

fn with_overrides(context: Context<'static>, overrides: &ContextOverrides) -> Context<'static> {
	if overrides.is_empty() {
		return context;
	}

	let context = context
		.as_deref()
		.map(|context| context.with_overrides(overrides))
		.unwrap_or_else(|| OwnedContextImpl::empty().with_overrides(overrides));
	Some(Arc::new(context))
}

fn context_features(context: &OwnedContextImpl) -> InspectedContext {
	use graphene_std::{ExtractAnimationTime, ExtractFootprint, ExtractIndex, ExtractPointerPosition, ExtractPosition, ExtractRealTime, ExtractVarArgs};

	InspectedContext {
		features: ContextOverrides {
			footprint: context.try_footprint().copied(),
			real_time: context.try_real_time(),
			animation_time: context.try_animation_time(),
			pointer_position: context.try_pointer_position(),
			index: context.try_index().map(|index| index.collect()),
		},
		positions: context.try_position().map(|positions| positions.collect()).unwrap_or_default(),
		variables: context.varargs_len().unwrap_or_default(),
		..Default::default()
	}
}
