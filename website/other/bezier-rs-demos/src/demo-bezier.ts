import { WasmBezier } from "@/../wasm/pkg";
import type { BezierFeatureKey } from "@/features-bezier";
import bezierFeatures from "@/features-bezier";
import type { InputOption } from "@/types";
import { getConstructorKey, getCurveType, MANIPULATOR_KEYS_FROM_BEZIER_TYPE } from "@/types";

export function demoBezier(title: string, points: number[][], key: BezierFeatureKey, inputOptions: InputOption[], triggerOnMouseMove: boolean) {
	const data = {
		element: document.createElement("div"),
		title,
		inputOptions,
		bezier: WasmBezier[getConstructorKey(getCurveType(points.length))](points),
		callback: bezierFeatures[key].callback,
		manipulatorKeys: MANIPULATOR_KEYS_FROM_BEZIER_TYPE[getCurveType(points.length)],
		activePointIndex: undefined as number | undefined,
		sliderData: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.default }))),
		sliderUnits: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.unit }))),
		locked: false,
		updateDemoSVG,
		onMouseDown,
		onMouseMove,
		onMouseUp,
	};

	function updateDemoSVG(figure: HTMLElement, mouseLocation?: [number, number]) {
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

	function onMouseMove(e: MouseEvent) {
		if (data.locked || !(e.currentTarget instanceof HTMLElement)) return;
		data.locked = true;

		if (data.activePointIndex !== undefined) {
			data.bezier[data.manipulatorKeys[data.activePointIndex]](e.offsetX, e.offsetY);
			points[data.activePointIndex] = [e.offsetX, e.offsetY];

			updateDemoSVG(e.currentTarget);
		} else if (triggerOnMouseMove) {
			updateDemoSVG(e.currentTarget, [e.offsetX, e.offsetY]);
		}

		data.locked = false;
	}

	function onMouseUp() {
		data.activePointIndex = undefined;
	}

	return data;
}
