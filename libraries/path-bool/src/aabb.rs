// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::vector::Vector;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AABB {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

pub fn bounding_box_contains_point(bounding_box: &AABB, point: Vector) -> bool {
    point.x >= bounding_box.left
        && point.x <= bounding_box.right
        && point.y >= bounding_box.top
        && point.y <= bounding_box.bottom
}

pub fn bounding_boxes_overlap(a: &AABB, b: &AABB) -> bool {
    a.left <= b.right && b.left <= a.right && a.top <= b.bottom && b.top <= a.bottom
}

pub fn merge_bounding_boxes(a: Option<AABB>, b: &AABB) -> AABB {
    match a {
        Some(a) => AABB {
            top: a.top.min(b.top),
            right: a.right.max(b.right),
            bottom: a.bottom.max(b.bottom),
            left: a.left.min(b.left),
        },
        None => *b,
    }
}

pub fn extend_bounding_box(bounding_box: Option<AABB>, point: Vector) -> AABB {
    match bounding_box {
        Some(bb) => AABB {
            top: bb.top.min(point.y),
            right: bb.right.max(point.x),
            bottom: bb.bottom.max(point.y),
            left: bb.left.min(point.x),
        },
        None => AABB {
            top: point.y,
            right: point.x,
            bottom: point.y,
            left: point.x,
        },
    }
}

pub fn bounding_box_max_extent(bounding_box: &AABB) -> f64 {
    (bounding_box.right - bounding_box.left).max(bounding_box.bottom - bounding_box.top)
}

pub fn bounding_box_around_point(point: Vector, padding: f64) -> AABB {
    AABB {
        top: point.y - padding,
        right: point.x + padding,
        bottom: point.y + padding,
        left: point.x - padding,
    }
}

pub fn expand_bounding_box(bounding_box: &AABB, padding: f64) -> AABB {
    AABB {
        top: bounding_box.top - padding,
        right: bounding_box.right + padding,
        bottom: bounding_box.bottom + padding,
        left: bounding_box.left - padding,
    }
}
