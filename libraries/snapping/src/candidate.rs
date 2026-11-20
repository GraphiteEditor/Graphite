use crate::locus::Locus;
use crate::scene::LayerId;
use glam::DVec2;

/// The relationship a candidate offers. The order is from most to least specific, and it settles ties between candidates that are equally near.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
	Anchor,
	Midpoint,
	Between,
	TangentPoint,
	PerpendicularFoot,
	BoxCorner,
	BoxEdgeMidpoint,
	BoxCenter,
	ArtboardCorner,
	ArtboardCenter,
	OnCurve,
	OnBoxEdge,
	OnArtboardEdge,
	Extension,
	TangentLine,
	PerpendicularLine,
	Parallel,
	RightAngle,
	Through,
	CommonTangent,
	Alignment,
	EqualLength,
	AngleStep,
	Spacing,
	EqualSize,
	EqualRadius,
	WholeValue,
}

/// Identifies a candidate from one moment to the next, so an engaged snap can be recognized again after the pointer moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CandidateKey {
	pub kind: Kind,
	/// Which feature of the moving thing the candidate applies to.
	pub feature: u32,
	/// Whatever tells this candidate apart from others of its kind, such as a layer and an index within it.
	pub identity: [u64; 3],
}

/// Where a candidate lets a feature be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
	/// One position, which fixes the feature completely.
	Spot(DVec2),
	/// A path the feature may slide along.
	Track(Locus),
}

/// The piece of the scene a candidate belongs to, for the presenter to highlight once it engages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Highlight {
	Anchor { layer: LayerId, index: u32 },
	Curve { layer: LayerId, index: u32 },
	Box { layer: LayerId },
}

/// Two gaps a spacing candidate makes equal, for the presenter to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchedGaps {
	/// The length both gaps have.
	pub gap: f64,
	/// Each gap as the two facing positions it spans, in the work plane.
	pub spans: [[DVec2; 2]; 2],
}

/// One way a feature could snap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
	pub key: CandidateKey,
	pub shape: Shape,
	pub highlight: Option<Highlight>,
	/// A position the relationship is measured from, such as the point being aligned with, for the presenter to connect to the feature.
	pub reference: Option<DVec2>,
	pub gaps: Option<MatchedGaps>,
}

impl Candidate {
	pub fn spot(key: CandidateKey, position: DVec2) -> Self {
		Self {
			key,
			shape: Shape::Spot(position),
			highlight: None,
			reference: None,
			gaps: None,
		}
	}

	pub fn track(key: CandidateKey, locus: Locus) -> Self {
		Self {
			key,
			shape: Shape::Track(locus),
			highlight: None,
			reference: None,
			gaps: None,
		}
	}

	pub fn with_highlight(self, highlight: Highlight) -> Self {
		Self { highlight: Some(highlight), ..self }
	}

	pub fn with_reference(self, reference: DVec2) -> Self {
		Self { reference: Some(reference), ..self }
	}

	pub fn with_gaps(self, gaps: MatchedGaps) -> Self {
		Self { gaps: Some(gaps), ..self }
	}
}
