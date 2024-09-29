import { WasmBezier } from "@/../wasm/pkg";
import type { BezierFeatureKey } from "@/features/bezier-features";
import bezierFeatures from "@/features/bezier-features";
import { renderDemo } from "@/utils/render";
import type { BezierCallback, BezierCurveType, InputOption, WasmBezierManipulatorKey } from "@/utils/types";
import { getConstructorKey, getCurveType } from "@/utils/types";

// Given the number of points in the curve, map the index of a point to the correct manipulator key
const MANIPULATOR_KEYS_FROM_BEZIER_TYPE: { [key in BezierCurveType]: WasmBezierManipulatorKey[] } = {
	Linear: ["set_start", "set_end"],
	Quadratic: ["set_start", "set_handle_start", "set_end"],
	Cubic: ["set_start", "set_handle_start", "set_handle_end", "set_end"],
};

export function newBezierDemo(title: string, points: number[][], key: BezierFeatureKey, inputOptions: InputOption[], triggerOnMouseMove: boolean) {
	// Avoids "recursive use of an object detected which would lead to unsafe aliasing in rust" error when moving mouse fast.
	let locked = false;

	const curveType = getCurveType(points.length);

	const data = {
		element: document.createElement("div"),
		title,
		inputOptions,
		bezier: WasmBezier[getConstructorKey(curveType)](points),
		callback: bezierFeatures[key].callback as BezierCallback,
		manipulatorKeys: MANIPULATOR_KEYS_FROM_BEZIER_TYPE[curveType],
		activePointIndex: undefined as number | undefined,
		sliderData: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.default }))),
		sliderUnits: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.unit }))),
		drawDemo,
		onMouseDown,
		onMouseUp,
		onMouseMove,
		getSliderUnit,
	};

	renderDemo(data);
	const figure = data.element.querySelector("figure") as HTMLElement;
	drawDemo(figure);

	function drawDemo(figure: HTMLElement, mouseLocation?: [number, number]) {
		figure.innerHTML = data.callback(data.bezier, data.sliderData, mouseLocation);
	}

	function onMouseDown(e: MouseEvent) {
		const SELECTABLE_RANGE = 10;

		const mx = e.offsetX;
		const my = e.offsetY;
		for (let pointIndex = 0; pointIndex < points.length; pointIndex += 1) {
			const point = points[pointIndex];
			if (point && Math.abs(mx - point[0]) < SELECTABLE_RANGE && Math.abs(my - point[1]) < SELECTABLE_RANGE) {
				data.activePointIndex = pointIndex;
				return;
			}
		}
	}

	function onMouseUp() {
		data.activePointIndex = undefined;
	}

	function onMouseMove(e: MouseEvent) {
		if (locked) return;
		locked = true;
		const mx = e.offsetX;
		const my = e.offsetY;
		const figure = e.currentTarget as HTMLElement;

		if (data.activePointIndex !== undefined) {
			data.bezier[data.manipulatorKeys[data.activePointIndex]](mx, my);
			points[data.activePointIndex] = [mx, my];
			drawDemo(figure);
		} else if (triggerOnMouseMove) {
			drawDemo(figure, [mx, my]);
		}
		locked = false;
	}

	function getSliderUnit(variable: string): string {
		const sliderUnit = data.sliderUnits[variable];
		return (Array.isArray(sliderUnit) ? "" : sliderUnit) || "";
	}

	return data;
}
