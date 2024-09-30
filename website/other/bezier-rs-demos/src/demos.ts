import { WasmSubpath, WasmBezier } from "@/../wasm/pkg";
import type { BezierFeatureKey } from "@/features-bezier";
import bezierFeatures from "@/features-bezier";
import type { SubpathFeatureKey } from "@/features-subpath";
import subpathFeatures from "@/features-subpath";
import type { WasmSubpathInstance, WasmSubpathManipulatorKey, InputOption, DemoData, DemoDataBezier, DemoDataSubpath } from "@/types";
import { POINT_INDEX_TO_MANIPULATOR, getConstructorKey, getCurveType, MANIPULATOR_KEYS_FROM_BEZIER_TYPE } from "@/types";

export function demoBezier(title: string, points: number[][], key: BezierFeatureKey, inputOptions: InputOption[], triggerOnMouseMove: boolean): DemoDataBezier {
	return {
		kind: "bezier",
		title,
		element: document.createElement("div"),
		inputOptions,
		locked: false,
		triggerOnMouseMove,
		sliderData: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.default }))),
		sliderUnits: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.unit }))),
		activePointIndex: undefined as number | undefined,
		manipulatorKeys: MANIPULATOR_KEYS_FROM_BEZIER_TYPE[getCurveType(points.length)],
		bezier: WasmBezier[getConstructorKey(getCurveType(points.length))](points),
		points,
		callback: bezierFeatures[key].callback,
	};
}

export function demoSubpath(title: string, triples: (number[] | undefined)[][], key: SubpathFeatureKey, closed: boolean, inputOptions: InputOption[], triggerOnMouseMove: boolean): DemoDataSubpath {
	return {
		kind: "subpath",
		title,
		element: document.createElement("div"),
		inputOptions,
		locked: false,
		triggerOnMouseMove,
		sliderData: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.default }))),
		sliderUnits: Object.assign({}, ...inputOptions.map((s) => ({ [s.variable]: s.unit }))),
		activePointIndex: undefined as number | undefined,
		activeManipulatorIndex: undefined as number | undefined,
		manipulatorKeys: undefined as undefined | WasmSubpathManipulatorKey[],
		subpath: WasmSubpath.from_triples(triples, closed) as WasmSubpathInstance,
		triples,
		callback: subpathFeatures[key].callback,
	};
}

export function updateDemoSVG(data: DemoData, figure: HTMLElement, mouseLocation?: [number, number]) {
	if (data.kind === "subpath") figure.innerHTML = data.callback(data.subpath, data.sliderData, mouseLocation);
	if (data.kind === "bezier") figure.innerHTML = data.callback(data.bezier, data.sliderData, mouseLocation);
}

export function onMouseDown(data: DemoData, e: MouseEvent) {
	const SELECTABLE_RANGE = 10;

	if (data.kind === "bezier") {
		const SELECTABLE_RANGE = 10;

		const distances = data.points.flatMap((point, pointIndex) => {
			if (!point) return [];
			const distance = Math.sqrt(Math.pow(e.offsetX - point[0], 2) + Math.pow(e.offsetY - point[1], 2));
			return distance < SELECTABLE_RANGE ? [{ pointIndex, distance }] : [];
		});
		const closest = distances.sort((a, b) => a.distance - b.distance)[0];
		if (closest) data.activePointIndex = closest.pointIndex;
	}

	if (data.kind === "subpath") {
		const distances = data.triples.flatMap((triple, manipulatorIndex) =>
			triple.flatMap((point, pointIndex) => {
				if (!point) return [];
				const distance = Math.sqrt(Math.pow(e.offsetX - point[0], 2) + Math.pow(e.offsetY - point[1], 2));
				return distance < SELECTABLE_RANGE ? [{ manipulatorIndex, pointIndex, distance }] : [];
			}),
		);
		const closest = distances.sort((a, b) => a.distance - b.distance)[0];
		if (closest) {
			data.activeManipulatorIndex = closest.manipulatorIndex;
			data.activePointIndex = closest.pointIndex;
		}
	}
}

export function onMouseMove(data: DemoData, e: MouseEvent) {
	if (data.locked || !(e.currentTarget instanceof HTMLElement)) return;
	data.locked = true;

	if (data.kind === "bezier") {
		if (data.activePointIndex !== undefined) {
			data.bezier[data.manipulatorKeys[data.activePointIndex]](e.offsetX, e.offsetY);
			data.points[data.activePointIndex] = [e.offsetX, e.offsetY];

			updateDemoSVG(data, e.currentTarget);
		} else if (data.triggerOnMouseMove) {
			updateDemoSVG(data, e.currentTarget, [e.offsetX, e.offsetY]);
		}
	}

	if (data.kind === "subpath") {
		if (data.activeManipulatorIndex !== undefined && data.activePointIndex !== undefined) {
			data.subpath[POINT_INDEX_TO_MANIPULATOR[data.activePointIndex]](data.activeManipulatorIndex, e.offsetX, e.offsetY);
			data.triples[data.activeManipulatorIndex][data.activePointIndex] = [e.offsetX, e.offsetY];

			updateDemoSVG(data, e.currentTarget);
		} else if (data.triggerOnMouseMove) {
			updateDemoSVG(data, e.currentTarget, [e.offsetX, e.offsetY]);
		}
	}

	data.locked = false;
}

export function onMouseUp(data: DemoData) {
	data.activePointIndex = undefined;
	if (data.kind === "subpath") data.activeManipulatorIndex = undefined;
}
