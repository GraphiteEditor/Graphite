use crate::{Ctx, ExtractIndex};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Hash, dyn_any::DynAny, Default)]
pub enum IndexOperationFilter {
	Range(Vec<core::ops::RangeInclusive<usize>>),
	#[default]
	All,
}

impl IndexOperationFilter {
	pub fn contains(&self, index: usize) -> bool {
		match self {
			Self::Range(range) => range.iter().any(|range| range.contains(&index)),
			Self::All => true,
		}
	}
}

impl From<Vec<core::ops::RangeInclusive<usize>>> for IndexOperationFilter {
	fn from(values: Vec<core::ops::RangeInclusive<usize>>) -> Self {
		Self::Range(values)
	}
}

impl From<core::ops::RangeInclusive<usize>> for IndexOperationFilter {
	fn from(value: core::ops::RangeInclusive<usize>) -> Self {
		Self::Range(vec![value])
	}
}

impl core::fmt::Display for IndexOperationFilter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::All => {
				write!(f, "*")?;
			}
			Self::Range(range) => {
				let mut started = false;
				for value in range {
					if started {
						write!(f, ", ")?;
					}
					started = true;
					if value.start() == value.end() {
						write!(f, "{}", value.start())?;
					} else {
						write!(f, "{}..={}", value.start(), value.end())?;
					}
				}
			}
		}

		Ok(())
	}
}

#[node_macro::node(category("Filtering"), path(graphene_core::vector))]
async fn evaluate_index_operation_filter(ctx: impl Ctx + ExtractIndex, filter: IndexOperationFilter) -> bool {
	let index = ctx.try_index().and_then(|indexes| indexes.last().copied()).unwrap_or_default();
	filter.contains(index)
}
