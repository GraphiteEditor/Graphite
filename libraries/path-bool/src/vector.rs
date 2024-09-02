// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use glam::DVec2;

pub type Vector = DVec2;

pub fn vectors_equal(a: Vector, b: Vector, eps: f64) -> bool {
	a.abs_diff_eq(b, eps)
}
