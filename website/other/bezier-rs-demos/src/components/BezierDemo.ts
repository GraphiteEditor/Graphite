import { WasmBezier } from "@/../wasm/pkg";
import type { BezierFeatureKey } from "@/features/bezier-features";
import bezierFeatures from "@/features/bezier-features";
import { renderDemo } from "@/utils/render";
import type { BezierCallback, BezierCurveType, InputOption, WasmBezierManipulatorKey, Demo } from "@/utils/types";
import { getConstructorKey, getCurveType } from "@/utils/types";

const SELECTABLE_RANGE = 10;

// Given the number of points in the curve, map the index of a point to the correct manipulator key
const MANIPULATOR_KEYS_FROM_BEZIER_TYPE: { [key in BezierCurveType]: WasmBezierManipulatorKey[] } = {
	Linear: ["set_start", "set_end"],
	Quadratic: ["set_start", "set_handle_start", "set_end"],
	Cubic: ["set_start", "set_handle_start", "set_handle_end", "set_end"],
};

class BezierDemo extends HTMLElement implements Demo {
	// Props
	title!: string;

	points!: number[][];

	key!: BezierFeatureKey;

	inputOptions!: InputOption[];

	triggerOnMouseMove!: boolean;

	// Data
	bezier!: WasmBezier;

	callback!: BezierCallback;

	manipulatorKeys!: WasmBezierManipulatorKey[];

	activeIndex!: number | undefined;

	sliderData!: Record<string, number>;

	sliderUnits!: Record<string, string | string[]>;

	// Avoids "recursive use of an object detected which would lead to unsafe aliasing in rust" error when moving mouse fast.
	locked!: boolean;

	async connectedCallback() {
		this.title = this.getAttribute("title") || "";
		this.points = JSON.parse(this.getAttribute("points") || "[]");
		this.key = this.getAttribute("key") as BezierFeatureKey;
		this.inputOptions = JSON.parse(this.getAttribute("inputOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";

		this.callback = bezierFeatures[this.key].callback as BezierCallback;
		const curveType = getCurveType(this.points.length);

		this.manipulatorKeys = MANIPULATOR_KEYS_FROM_BEZIER_TYPE[curveType];
		this.activeIndex = undefined as number | undefined;
		this.sliderData = Object.assign({}, ...this.inputOptions.map((s) => ({ [s.variable]: s.default })));
		this.sliderUnits = Object.assign({}, ...this.inputOptions.map((s) => ({ [s.variable]: s.unit })));
		this.render();

		const figure = this.querySelector("figure") as HTMLElement;
		this.bezier = WasmBezier[getConstructorKey(curveType)](this.points);
		this.drawDemo(figure);
	}

	render() {
		renderDemo(this);
	}

	drawDemo(figure: HTMLElement, mouseLocation?: [number, number]) {
		figure.innerHTML = this.callback(this.bezier, this.sliderData, mouseLocation);
	}

	onMouseDown(event: MouseEvent) {
		const mx = event.offsetX;
		const my = event.offsetY;
		for (let pointIndex = 0; pointIndex < this.points.length; pointIndex += 1) {
			const point = this.points[pointIndex];
			if (point && Math.abs(mx - point[0]) < SELECTABLE_RANGE && Math.abs(my - point[1]) < SELECTABLE_RANGE) {
				this.activeIndex = pointIndex;
				return;
			}
		}
	}

	onMouseUp() {
		this.activeIndex = undefined;
	}

	onMouseMove(event: MouseEvent) {
		if (this.locked) return;
		this.locked = true;
		const mx = event.offsetX;
		const my = event.offsetY;
		const figure = event.currentTarget as HTMLElement;

		if (this.activeIndex !== undefined) {
			this.bezier[this.manipulatorKeys[this.activeIndex]](mx, my);
			this.points[this.activeIndex] = [mx, my];
			this.drawDemo(figure);
		} else if (this.triggerOnMouseMove) {
			this.drawDemo(figure, [mx, my]);
		}
		this.locked = false;
	}

	getSliderUnit(sliderValue: number, variable: string): string {
		const sliderUnit = this.sliderUnits[variable];
		return (Array.isArray(sliderUnit) ? "" : sliderUnit) || "";
	}
}

export default BezierDemo;
