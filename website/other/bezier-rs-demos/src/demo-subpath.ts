import { WasmSubpath } from "@/../wasm/pkg";
import type { SubpathFeatureKey } from "@/features-subpath";
import subpathFeatures from "@/features-subpath";
import { renderDemo } from "@/main";
import { type WasmSubpathInstance, type WasmSubpathManipulatorKey, type InputOption, POINT_INDEX_TO_MANIPULATOR } from "@/types";

export function demoSubpath(title: string, triples: (number[] | undefined)[][], key: SubpathFeatureKey, closed: boolean, inputOptions: InputOption[], triggerOnMouseMove: boolean) {
	const data = {
		element: document.createElement("div"),
		title,
		inputOptions,
		subpath: WasmSubpath.from_triples(triples, closed) as WasmSubpathInstance,
		callback: subpathFeatures[key].callback,
		manipulatorKeys: undefined as undefined | WasmSubpathManipulatorKey[],
		activeManipulatorIndex: undefined as number | undefined,
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

	// Methods

	function drawDemo(figure: HTMLElement, mouseLocation?: [number, number]) {
		figure.innerHTML = data.callback(data.subpath, data.sliderData, mouseLocation);
	}

	function onMouseDown(e: MouseEvent) {
		const SELECTABLE_RANGE = 10;

		const distances = triples.flatMap((triple, manipulatorIndex) =>
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

	function onMouseUp() {
		data.activeManipulatorIndex = undefined;
		data.activePointIndex = undefined;
	}

	let locked = false;
	function onMouseMove(e: MouseEvent) {
		if (locked || !(e.currentTarget instanceof HTMLElement)) return;
		locked = true;

		if (data.activeManipulatorIndex !== undefined && data.activePointIndex !== undefined) {
			data.subpath[POINT_INDEX_TO_MANIPULATOR[data.activePointIndex]](data.activeManipulatorIndex, e.offsetX, e.offsetY);
			triples[data.activeManipulatorIndex][data.activePointIndex] = [e.offsetX, e.offsetY];

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
