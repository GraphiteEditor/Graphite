import { WasmSubpath } from "@/../wasm/pkg";
import type { SubpathFeatureKey } from "@/features/subpath-features";
import subpathFeatures from "@/features/subpath-features";
import { renderDemo } from "@/utils/render";
import type { WasmSubpathInstance, WasmSubpathManipulatorKey, InputOption } from "@/utils/types";

const POINT_INDEX_TO_MANIPULATOR: WasmSubpathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

export function newSubpathDemo(title: string, triples: (number[] | undefined)[][], key: SubpathFeatureKey, closed: boolean, inputOptions: InputOption[], triggerOnMouseMove: boolean) {
	const data = {
		element: document.createElement("div"),
		title,
		inputOptions,
		subpath: WasmSubpath.from_triples(triples, closed) as WasmSubpathInstance,
		callback: subpathFeatures[key].callback,
		manipulatorKeys: undefined as undefined | WasmSubpathManipulatorKey[],
		activeControllerIndex: undefined as number | undefined,
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
		figure.innerHTML = data.callback(data.subpath, data.sliderData, mouseLocation);
	}

	function onMouseDown(event: MouseEvent) {
		const SELECTABLE_RANGE = 10;

		const mx = event.offsetX;
		const my = event.offsetY;
		for (let controllerIndex = 0; controllerIndex < triples.length; controllerIndex += 1) {
			for (let pointIndex = 0; pointIndex < 3; pointIndex += 1) {
				const point = triples[controllerIndex][pointIndex];
				if (point && Math.abs(mx - point[0]) < SELECTABLE_RANGE && Math.abs(my - point[1]) < SELECTABLE_RANGE) {
					data.activeControllerIndex = controllerIndex;
					data.activePointIndex = pointIndex;
					return;
				}
			}
		}
	}

	function onMouseUp() {
		data.activeControllerIndex = undefined;
		data.activePointIndex = undefined;
	}

	function onMouseMove(event: MouseEvent) {
		const mx = event.offsetX;
		const my = event.offsetY;
		const figure = event.currentTarget as HTMLElement;
		if (data.activeControllerIndex !== undefined && data.activePointIndex !== undefined) {
			data.subpath[POINT_INDEX_TO_MANIPULATOR[data.activePointIndex]](data.activeControllerIndex, mx, my);
			triples[data.activeControllerIndex][data.activePointIndex] = [mx, my];
			drawDemo(figure);
		} else if (triggerOnMouseMove) {
			drawDemo(figure, [mx, my]);
		}
	}

	function getSliderUnit(variable: string): string {
		const sliderUnit = data.sliderUnits[variable];
		return (Array.isArray(sliderUnit) ? "" : sliderUnit) || "";
	}

	return data;
}
