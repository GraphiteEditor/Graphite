import { WasmBezier } from "@/../wasm/pkg";
import bezierFeatures, { BezierFeatureName } from "@/features/bezierFeatures";
import { renderExample } from "@/utils/render";
import { getConstructorKey, getCurveType, BezierCallback, BezierCurveType, SliderOption, WasmBezierManipulatorKey, ComputeType, Example } from "@/utils/types";

const SELECTABLE_RANGE = 10;

// Given the number of points in the curve, map the index of a point to the correct manipulator key
const MANIPULATOR_KEYS_FROM_BEZIER_TYPE: { [key in BezierCurveType]: WasmBezierManipulatorKey[] } = {
	Linear: ["set_start", "set_end"],
	Quadratic: ["set_start", "set_handle_start", "set_end"],
	Cubic: ["set_start", "set_handle_start", "set_handle_end", "set_end"],
};

class BezierExample extends HTMLElement implements Example {
	// Props
	title!: string;

	points!: number[][];

	name!: BezierFeatureName;

	sliderOptions!: SliderOption[];

	triggerOnMouseMove!: boolean;

	computeType!: ComputeType;

	// Data
	bezier!: WasmBezier;

	callback!: BezierCallback;

	manipulatorKeys!: WasmBezierManipulatorKey[];

	activeIndex!: number | undefined;

	sliderData!: Record<string, number>;

	sliderUnits!: Record<string, string | string[]>;

	static get observedAttributes(): string[] {
		return ["computetype"];
	}

	attributeChangedCallback(name: string, oldValue: string, newValue: string): void {
		if (name === "computetype" && oldValue) {
			this.computeType = (newValue || "Parametric") as ComputeType;
			const figure = this.querySelector("figure") as HTMLElement;
			this.drawExample(figure);
		}
	}

	connectedCallback(): void {
		this.title = this.getAttribute("title") || "";
		this.points = JSON.parse(this.getAttribute("points") || "[]");
		this.name = this.getAttribute("name") as BezierFeatureName;
		this.sliderOptions = JSON.parse(this.getAttribute("sliderOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.computeType = (this.getAttribute("computetype") || "Parametric") as ComputeType;

		this.callback = bezierFeatures[this.name].callback as BezierCallback;
		const curveType = getCurveType(this.points.length);

		this.manipulatorKeys = MANIPULATOR_KEYS_FROM_BEZIER_TYPE[curveType];
		this.bezier = WasmBezier[getConstructorKey(curveType)](this.points);
		this.activeIndex = undefined as number | undefined;
		this.sliderData = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.default })));
		this.sliderUnits = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.unit })));
		this.render();

		const figure = this.querySelector("figure") as HTMLElement;
		this.drawExample(figure);
	}

	render(): void {
		renderExample(this);
	}

	drawExample(figure: HTMLElement, mouseLocation?: [number, number]): void {
		figure.innerHTML = this.callback(this.bezier, this.sliderData, mouseLocation, this.computeType);
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
			this.drawExample(figure);
		} else if (this.triggerOnMouseMove) {
			this.drawExample(figure, [mx, my]);
		}
	}

	getSliderUnit(sliderValue: number, variable: string): string {
		const sliderUnit = this.sliderUnits[variable];
		return (Array.isArray(sliderUnit) ? sliderUnit[sliderValue] : sliderUnit) || "";
	}
}

export default BezierExample;
