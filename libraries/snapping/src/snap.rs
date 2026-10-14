use crate::candidate::{Candidate, Kind, Shape};
use crate::find::{Context, Probe, find_near_probe};
use crate::grid::Grid;
use crate::locus::Locus;
use crate::solve::{CrossingCache, Engagement, FeatureMap, Incumbent, Problem, Snap, confine, solve};
use glam::DVec2;
use std::collections::{HashMap, HashSet};

/// The parts of a snap attempt that carry over from one moment of a gesture to the next, or that the user controls directly.
#[derive(Clone, Copy, Debug, Default)]
pub struct Request<'a> {
	/// The grid to land on while grid snapping is on.
	pub grid: Option<&'a Grid>,
	/// A track that must be stayed on, in the same terms as the result: positions when placing a point, or amounts slid.
	pub required: Option<Locus>,
	/// What was engaged a moment ago.
	pub incumbent: Option<Incumbent>,
	/// How many candidates to step on from the one that would otherwise be chosen. It applies to this one attempt, and what it reaches comes back pinned.
	pub cycle: usize,
	/// Whether snapping is suspended, which leaves only the required track in effect.
	pub quiet: bool,
	/// Candidates that do not depend on where the pointer is, offered to every feature.
	pub constructions: &'a [Candidate],
	/// Candidates offered to the lead feature alone, such as those from the point it joins back to.
	pub lead_constructions: &'a [Candidate],
	/// Candidates the caller found for particular features, such as the places that even out a box's gaps, each for the feature its key names.
	pub particular: &'a [Candidate],
	/// The features that look for geometry near them and are offered the constructions, which is all of them when `None`.
	/// One left out still follows the parameters, can be the lead, and can be given `particular` candidates.
	pub probed: Option<&'a [u32]>,
	/// Somewhere to remember the crossings of curves between attempts, which spares finding them again at every move of the pointer.
	pub crossings: Option<&'a CrossingCache>,
}

/// What a snap attempt came to.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
	/// The snapped result: a position when placing a point, or the amount everything has slid.
	pub parameters: DVec2,
	pub snap: Snap,
	/// Every candidate that was considered, in the coordinates of the work plane, which an engagement's indices point into.
	pub candidates: Vec<Candidate>,
}

impl Outcome {
	pub fn engagement(&self) -> Option<&Engagement> {
		match &self.snap {
			Snap::Engaged(engagement) => Some(engagement),
			_ => None,
		}
	}

	/// The candidates that are engaged: one, or the two tracks of a crossing.
	pub fn engaged_candidates(&self) -> impl Iterator<Item = &Candidate> {
		let indices = self.engagement().map(|engagement| engagement.candidates.as_slice()).unwrap_or_default();
		indices.iter().filter_map(|&index| self.candidates.get(index))
	}

	/// What to pass as the incumbent a moment later so the engaged snap holds on.
	pub fn incumbent(&self) -> Option<Incumbent> {
		self.engagement().map(|engagement| Incumbent {
			key: engagement.key,
			parameters: self.parameters,
			pinned: engagement.pinned,
			riding: engagement.riding,
		})
	}
}

/// Snaps a single point being placed or dragged, such as an anchor or the end of a line.
///
/// `previous` is the point it joins back to with a straight segment, if any, which it is kept from landing on.
pub fn snap_point(context: &Context, request: &Request, free: DVec2, previous: Option<DVec2>) -> Outcome {
	snap_features(context, request, &[FeatureMap::IDENTITY], 0, free, previous)
}

/// Snaps a set of features sliding together, given where each began and how far the pointer asks them to slide.
///
/// The feature at index `lead` is the one the pointer carries, which lands on the grid.
pub fn snap_slide(context: &Context, request: &Request, feature_starts: &[DVec2], lead: usize, free: DVec2) -> Outcome {
	let features: Vec<FeatureMap> = feature_starts.iter().map(|&start| FeatureMap::sliding_from(start)).collect();
	snap_features(context, request, &features, lead, free, None)
}

/// Snaps a gesture in which each feature follows the two parameters in its own way, such as a line growing both ways from its center.
///
/// The features must keep their order for as long as the gesture lasts, since an engaged snap is recognized by the index of its feature.
/// `previous` is the point the lead feature joins back to with a straight segment, if any.
pub fn snap_features(context: &Context, request: &Request, features: &[FeatureMap], lead: usize, free: DVec2, previous: Option<DVec2>) -> Outcome {
	// Under a required track, what matters is near where the pointer falls on it rather than near the pointer
	let lead_feature = features.get(lead).copied().unwrap_or(FeatureMap::IDENTITY);
	let base = confine(context.view, &lead_feature, request.required.as_ref(), free);

	if request.quiet || !base.is_finite() {
		return Outcome {
			parameters: if base.is_finite() { base } else { free },
			snap: Snap::Free,
			candidates: Vec::new(),
		};
	}

	// Each feature gathers its own candidates where the pointer would put it, alongside the ones offered wherever it is
	let mut candidates = Vec::new();
	for (index, feature) in features.iter().enumerate() {
		let index = index as u32;
		if request.probed.is_some_and(|probed| !probed.contains(&index)) {
			continue;
		}

		let is_lead = index as usize == lead;
		let probe = Probe {
			feature: index,
			position: feature.position(base),
			previous: if is_lead { previous } else { None },
		};
		find_near_probe(context, &probe, &mut candidates);

		let lead_constructions = if is_lead { request.lead_constructions } else { &[] };
		candidates.extend(request.constructions.iter().chain(lead_constructions).map(|construction| {
			let mut key = construction.key;
			key.feature = index;
			Candidate { key, ..*construction }
		}));
	}

	let exists = |candidate: &&Candidate| (candidate.key.feature as usize) < features.len();
	candidates.extend(request.particular.iter().filter(exists));

	// One relationship can be offered twice over, as by a curve that is both awake and near the pointer
	if !request.constructions.is_empty() || !request.lead_constructions.is_empty() || !request.particular.is_empty() {
		let mut offered = HashSet::new();
		candidates.retain(|candidate| offered.insert(candidate.key));
	}

	// The solver works in the gesture's parameters, where a candidate for one feature becomes one for the whole gesture.
	// A candidate with no counterpart there is kept as a spot nothing can reach, so that indices still line up.
	let in_parameters: Vec<Candidate> = candidates
		.iter()
		.map(|candidate| {
			let feature = features.get(candidate.key.feature as usize).copied().unwrap_or(FeatureMap::IDENTITY);

			let shape = match candidate.shape {
				Shape::Spot(position) => feature.parameters_for(position).map(Shape::Spot),
				Shape::Track(locus) => feature
					.parameters_for(DVec2::ZERO)
					.and_then(|origin| locus.transformed(feature.matrix.inverse(), origin))
					.map(Shape::Track),
			};
			Candidate {
				shape: shape.unwrap_or(Shape::Spot(DVec2::NAN)),
				..*candidate
			}
		})
		.collect();

	// Features in line with each other, such as a box's corners down one side, turn one relationship into copies that land alike.
	// One copy is kept, the lead's if it has one, so stepping through the snaps moves on to a different one. O(n) in the candidates.
	let lead_first = (0..candidates.len()).filter(|&index| candidates[index].key.feature as usize == lead);
	let others = (0..candidates.len()).filter(|&index| candidates[index].key.feature as usize != lead);
	let mut kept_copies: HashMap<(Kind, [u64; 3]), Vec<usize>> = HashMap::new();
	let mut keep = vec![false; candidates.len()];
	for index in lead_first.chain(others) {
		let key = candidates[index].key;
		let copies = kept_copies.entry((key.kind, key.identity)).or_default();
		if !copies.iter().any(|&kept| lands_alike(&in_parameters[kept].shape, &in_parameters[index].shape)) {
			copies.push(index);
			keep[index] = true;
		}
	}
	let (candidates, in_parameters): (Vec<Candidate>, Vec<Candidate>) = candidates.into_iter().zip(in_parameters).zip(keep).filter_map(|(pair, keep)| keep.then_some(pair)).unzip();

	let solution = solve(&Problem {
		view: context.view,
		tunables: context.tunables,
		features,
		lead: lead as u32,
		free,
		candidates: &in_parameters,
		required: request.required,
		grid: request.grid,
		incumbent: request.incumbent,
		cycle: request.cycle,
		crossings: request.crossings,
		avoid: previous.and_then(|previous| lead_feature.parameters_for(previous)),
	});

	Outcome {
		parameters: solution.parameters,
		snap: solution.snap,
		candidates,
	}
}

/// Whether two shapes in the gesture's parameters are the same to within rounding, so snapping to either lands alike.
fn lands_alike(a: &Shape, b: &Shape) -> bool {
	let close = |a: DVec2, b: DVec2| (a - b).abs().cmple(a.abs().max(b.abs()).max(DVec2::ONE) * 1e-9).all();

	match (a, b) {
		(Shape::Spot(a), Shape::Spot(b)) => close(*a, *b),
		(
			Shape::Track(Locus::Line { point, direction }),
			Shape::Track(Locus::Line {
				point: other_point,
				direction: other_direction,
			}),
		) => {
			let scale = point.abs().max(other_point.abs()).max_element().max(1.);
			direction.perp_dot(*other_direction).abs() <= 1e-9 && direction.perp_dot(*other_point - *point).abs() <= 1e-9 * scale
		}
		(Shape::Track(a), Shape::Track(b)) => a == b,
		_ => false,
	}
}
