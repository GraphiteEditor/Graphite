import { WasmBezier } from "@graphite/../wasm/pkg";
import bezierFeatures, { BezierFeatureKey } from "@graphite/features/bezier-features";
import { renderDemo } from "@graphite/utils/render";
import { getConstructorKey, getCurveType, BezierCallback, BezierCurveType, InputOption, WasmBezierManipulatorKey, Demo } from "@graphite/utils/types";

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

	async connectedCallback(): Promise<void> {
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
		const wasm = await import("@graphite/../wasm/pkg");
		this.bezier = wasm.WasmBezier[getConstructorKey(curveType)](this.points);
		this.drawDemo(figure);
	}

	render(): void {
		renderDemo(this);
	}

	drawDemo(figure: HTMLElement, mouseLocation?: [number, number]): void {
		figure.innerHTML = this.callback(this.bezier, this.sliderData, mouseLocation);
	}

	onMouseDown(event: MouseEvent): void {
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

	onMouseUp(): void {
		this.activeIndex = undefined;
	}

	onMouseMove(event: MouseEvent): void {
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
	}

	getSliderUnit(sliderValue: number, variable: string): string {
		const sliderUnit = this.sliderUnits[variable];
		return (Array.isArray(sliderUnit) ? "" : sliderUnit) || "";
	}
}

export default BezierDemo;
