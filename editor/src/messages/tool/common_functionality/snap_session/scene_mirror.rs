use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2};
use graphene_std::renderer::Quad;
use kurbo::{Affine, BezPath, PathEl, PathSeg};
use snapping::{AnchorId, Curve, GeometryRole, LayerId, LayerKind, Scene, SceneAnchor, SceneCurve, SceneLayer};
use std::collections::HashSet;

/// How the snapping library knows a layer of the document.
pub fn layer_id(layer: LayerNodeIdentifier) -> LayerId {
	LayerId(layer.to_node().0)
}

/// Brings a scene up to date with the document's layers as its latest render reported them, and returns the layers that changed or went.
/// O(n) in all the geometry of the document.
pub fn mirror(scene: &mut Scene, document: &DocumentMessageHandler) -> HashSet<LayerId> {
	let mut present = HashSet::new();
	let mut changed = HashSet::new();

	for layer in document.metadata().all_layers() {
		let Some(mirrored) = mirror_layer(document, layer) else { continue };
		let id = layer_id(layer);
		present.insert(id);

		// Most layers are the same from one render to the next, and telling so costs far less than indexing a layer afresh
		if scene.layer(id).is_none_or(|stored| !unchanged(stored, &mirrored)) {
			scene.set_layer(id, mirrored);
			changed.insert(id);
		}
	}

	let gone: Vec<LayerId> = scene.layers().map(|(id, _)| id).filter(|id| !present.contains(id)).collect();
	for id in gone {
		scene.remove_layer(id);
		changed.insert(id);
	}

	scene.prepare();
	changed
}

/// Whether a layer is as it was, allowing for the rounding that bringing its positions into document space through a different view leaves in them.
fn unchanged(stored: &SceneLayer, mirrored: &SceneLayer) -> bool {
	let close = |a: DVec2, b: DVec2| (a - b).abs().cmple(a.abs().max(DVec2::ONE) * 1e-9).all();

	let same_quad = match (stored.quad, mirrored.quad) {
		(Some(stored), Some(mirrored)) => stored.into_iter().zip(mirrored).all(|(a, b)| close(a, b)),
		(stored, mirrored) => stored.is_none() && mirrored.is_none(),
	};
	let same_anchors = stored.anchors.len() == mirrored.anchors.len()
		&& stored
			.anchors
			.iter()
			.zip(&mirrored.anchors)
			.all(|(a, b)| a.id == b.id && a.role == b.role && close(a.position, b.position));
	let same_curves = stored.curves.len() == mirrored.curves.len()
		&& stored.curves.iter().zip(&mirrored.curves).all(|(a, b)| {
			let same_order = std::mem::discriminant(&a.curve.segment()) == std::mem::discriminant(&b.curve.segment());
			a.ends == b.ends && a.role == b.role && same_order && control_points(&a.curve).into_iter().zip(control_points(&b.curve)).all(|(a, b)| close(a, b))
		});

	stored.parent == mirrored.parent && stored.kind == mirrored.kind && same_quad && same_anchors && same_curves
}

/// A curve's control points, with the end repeated for a curve of lower order.
fn control_points(curve: &Curve) -> [DVec2; 4] {
	let point = |point: kurbo::Point| DVec2::new(point.x, point.y);
	match curve.segment() {
		PathSeg::Line(line) => [line.p0, line.p1, line.p1, line.p1].map(point),
		PathSeg::Quad(quadratic) => [quadratic.p0, quadratic.p1, quadratic.p2, quadratic.p2].map(point),
		PathSeg::Cubic(cubic) => [cubic.p0, cubic.p1, cubic.p2, cubic.p3].map(point),
	}
}

/// Everything about one layer that can be snapped to, in document space. Returns `None` for a layer that offers nothing itself, such as a hidden one or a group.
fn mirror_layer(document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> Option<SceneLayer> {
	let (metadata, network_interface) = (document.metadata(), &document.network_interface);
	if !network_interface.is_layer_visible(layer) {
		return None;
	}

	let parent = layer.parent(metadata).filter(|&parent| parent != LayerNodeIdentifier::ROOT_PARENT).map(layer_id);
	let to_document = metadata.transform_to_document_as_rendered(layer);
	if !to_document.is_finite() {
		return None;
	}

	// An artboard offers the box it is drawn as, which stays upright
	if network_interface.is_artboard(&layer.to_node(), &[]) {
		let corners = Quad::from_box(metadata.bounding_box_with_transform(layer, to_document)?).0;
		return corners.iter().all(|corner| corner.is_finite()).then_some(SceneLayer {
			parent,
			kind: LayerKind::Artboard,
			quad: Some(corners),
			anchors: Vec::new(),
			curves: Vec::new(),
		});
	}

	// A group is snapped to by way of the layers inside it
	if layer.has_children(metadata) {
		return None;
	}

	let local_bounds = metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY);
	let quad = local_bounds
		.map(|bounds| (to_document * Quad::from_box(bounds)).0)
		.filter(|corners| corners.iter().all(|corner| corner.is_finite()));

	// What the layer draws
	let affine = Affine::new(to_document.to_cols_array());
	let outline: Vec<BezPath> = metadata.layer_outline(layer).collect();
	let drawn_curves: Vec<Curve> = outline.iter().flat_map(|path| path.segments()).map(|segment| Curve::new(affine * segment)).collect();
	let drawn_anchors: Vec<DVec2> = outline
		.iter()
		.flat_map(anchor_positions)
		.map(|position| to_document.transform_point2(position))
		.filter(|position| position.is_finite())
		.collect();

	let mut anchors = Vec::new();
	let mut curves = Vec::new();

	// What the Path tool edits, for a layer with a path to edit, which counts as drawn wherever the layer draws just that
	if let Some(vector) = network_interface.upstream_path_node_vector(layer) {
		let to_document = metadata.transform_to_document_as_rendered_if_feeds(layer, network_interface);

		if to_document.is_finite() {
			let affine = Affine::new(to_document.to_cols_array());
			let anchors_drawn: HashSet<[i64; 2]> = drawn_anchors.iter().map(|&position| rounded(position)).collect();
			let curves_drawn: HashSet<[[i64; 2]; 4]> = drawn_curves.iter().map(rounded_curve).collect();
			let role = |drawn: bool| if drawn { GeometryRole::Source } else { GeometryRole::Undrawn };

			anchors.extend(vector.anchors().map(|(id, position)| {
				let position = to_document.transform_point2(position);
				SceneAnchor {
					position,
					id: Some(AnchorId(id.inner())),
					role: role(anchors_drawn.contains(&rounded(position))),
				}
			}));
			curves.extend(vector.segment_iter().map(|(_, segment, start, end)| {
				let curve = Curve::new(affine * segment);
				SceneCurve {
					curve,
					ends: [Some(AnchorId(start.inner())), Some(AnchorId(end.inner()))],
					role: role(curves_drawn.contains(&rounded_curve(&curve))),
				}
			}));
		}
	}

	// The rest of what the layer draws, which is whatever does not just repeat the path above
	let mut anchors_seen: HashSet<[i64; 2]> = anchors.iter().map(|anchor| rounded(anchor.position)).collect();
	let curves_seen: HashSet<[[i64; 2]; 4]> = curves.iter().map(|curve| rounded_curve(&curve.curve)).collect();

	for curve in drawn_curves {
		if !curves_seen.contains(&rounded_curve(&curve)) {
			let (ends, role) = ([None, None], GeometryRole::Output);
			curves.push(SceneCurve { curve, ends, role });
		}
	}
	for position in drawn_anchors {
		if anchors_seen.insert(rounded(position)) {
			let (id, role) = (None, GeometryRole::Output);
			anchors.push(SceneAnchor { position, id, role });
		}
	}

	let offers_something = quad.is_some() || !anchors.is_empty() || !curves.is_empty();
	offers_something.then_some(SceneLayer {
		parent,
		kind: LayerKind::Artwork,
		quad,
		anchors,
		curves,
	})
}

/// The positions a path passes through at the ends of its segments.
pub fn anchor_positions(path: &BezPath) -> impl Iterator<Item = DVec2> + '_ {
	path.elements().iter().filter_map(|element| match *element {
		PathEl::MoveTo(point) | PathEl::LineTo(point) | PathEl::QuadTo(_, point) | PathEl::CurveTo(_, _, point) => Some(DVec2::new(point.x, point.y)),
		PathEl::ClosePath => None,
	})
}

/// A position to the nearest 1024th of a document unit, by which two that were reached in different ways are told to be the same.
fn rounded(position: DVec2) -> [i64; 2] {
	(position * 1024.).round().to_array().map(|coordinate| coordinate as i64)
}

/// A curve by its rounded control points, with a straight one given by its ends alone however it happens to be stored.
fn rounded_curve(curve: &Curve) -> [[i64; 2]; 4] {
	let [start, end] = [rounded(curve.start()), rounded(curve.end())];
	let point = |point: kurbo::Point| rounded(DVec2::new(point.x, point.y));

	match curve.segment() {
		_ if curve.is_straight() => [start, end, end, end],
		PathSeg::Line(_) => [start, end, end, end],
		PathSeg::Quad(quadratic) => [start, point(quadratic.p1), end, end],
		PathSeg::Cubic(cubic) => [start, point(cubic.p1), point(cubic.p2), end],
	}
}
