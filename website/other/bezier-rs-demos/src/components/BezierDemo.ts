import { WasmBezier } from "@/../wasm/pkg";
import type { BezierFeatureKey } from "@/features/bezier-features";
import bezierFeatures from "@/features/bezier-features";
import { renderDemo } from "@/utils/render";
import type { BezierCurveType, InputOption, WasmBezierManipulatorKey } from "@/utils/types";
import { getConstructorKey, getCurveType } from "@/utils/types";

// Given the number of points in the curve, map the index of a point to the correct manipulator key
const MANIPULATOR_KEYS_FROM_BEZIER_TYPE: { [key in BezierCurveType]: WasmBezierManipulatorKey[] } = {
	Linear: ["set_start", "set_end"],
	Quadratic: ["set_start", "set_handle_start", "set_end"],
	Cubic: ["set_start", "set_handle_start", "set_handle_end", "set_end"],
};

export function newBezierDemo(title: string, points: number[][], key: BezierFeatureKey, inputOptions: InputOption[], triggerOnMouseMove: boolean) {
	const curveType = getCurveType(points.length);

	const data = {
		element: document.createElement("div"),
		title,
		inputOptions,
		bezier: WasmBezier[getConstructorKey(curveType)](points),
		callback: bezierFeatures[key].callback,
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
	const figure = data.element.querySelector("[data-demo-figure]");
	if (figure instanceof HTMLElement) drawDemo(figure);

	function drawDemo(figure: HTMLElement, mouseLocation?: [number, number]) {
		figure.innerHTML = data.callback(data.bezier, data.sliderData, mouseLocation);
	}

	function onMouseDown(e: MouseEvent) {
		const SELECTABLE_RANGE = 10;

		const distances = points.flatMap((point, pointIndex) => {
			if (!point) return [];
			const distance = Math.sqrt(Math.pow(e.offsetX - point[0], 2) + Math.pow(e.offsetY - point[1], 2));
			return distance < SELECTABLE_RANGE ? [{ pointIndex, distance }] : [];
		});
		const closest = distances.sort((a, b) => a.distance - b.distance)[0];
		if (closest) data.activePointIndex = closest.pointIndex;
	}

	function onMouseUp() {
		data.activePointIndex = undefined;
	}

	let locked = false;
	function onMouseMove(e: MouseEvent) {
		if (locked || !(e.currentTarget instanceof HTMLElement)) return;
		locked = true;

		if (data.activePointIndex !== undefined) {
			data.bezier[data.manipulatorKeys[data.activePointIndex]](e.offsetX, e.offsetY);
			points[data.activePointIndex] = [e.offsetX, e.offsetY];

			drawDemo(e.currentTarget);
		} else if (triggerOnMouseMove) {
			drawDemo(e.currentTarget, [e.offsetX, e.offsetY]);
		}

		locked = false;
	}

	function getSliderUnit(variable: string): string {
		const sliderUnit = data.sliderUnits[variable];
		return (Array.isArray(sliderUnit) ? "" : sliderUnit) || "";
	}

	return data;
}
