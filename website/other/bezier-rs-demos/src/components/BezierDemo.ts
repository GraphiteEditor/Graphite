import { WasmBezier } from "@/../wasm/pkg";
import bezierFeatures, { BezierFeatureKey } from "@/features/bezier-features";
import { renderDemo } from "@/utils/render";
import { getConstructorKey, getCurveType, BezierCallback, BezierCurveType, SliderOption, WasmBezierManipulatorKey, TVariant, Demo } from "@/utils/types";

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

	sliderOptions!: SliderOption[];

	triggerOnMouseMove!: boolean;

	tVariant!: TVariant;

	// Data
	bezier!: WasmBezier;

	callback!: BezierCallback;

	manipulatorKeys!: WasmBezierManipulatorKey[];

	activeIndex!: number | undefined;

	sliderData!: Record<string, number>;

	sliderUnits!: Record<string, string | string[]>;

	static get observedAttributes(): string[] {
		return ["tvariant"];
	}

	attributeChangedCallback(name: string, oldValue: string, newValue: string): void {
		if (name === "tvariant" && oldValue) {
			this.tVariant = (newValue || "Parametric") as TVariant;
			const figure = this.querySelector("figure") as HTMLElement;
			this.drawDemo(figure);
		}
	}

	async connectedCallback(): Promise<void> {
		this.title = this.getAttribute("title") || "";
		this.points = JSON.parse(this.getAttribute("points") || "[]");
		this.key = this.getAttribute("key") as BezierFeatureKey;
		this.sliderOptions = JSON.parse(this.getAttribute("sliderOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.tVariant = (this.getAttribute("tvariant") || "Parametric") as TVariant;

		this.callback = bezierFeatures[this.key].callback as BezierCallback;
		const curveType = getCurveType(this.points.length);

		this.manipulatorKeys = MANIPULATOR_KEYS_FROM_BEZIER_TYPE[curveType];
		this.activeIndex = undefined as number | undefined;
		this.sliderData = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.default })));
		this.sliderUnits = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.unit })));
		this.render();

		const figure = this.querySelector("figure") as HTMLElement;
		const wasm = await import("@/../wasm/pkg");
		this.bezier = wasm.WasmBezier[getConstructorKey(curveType)](this.points);
		this.drawDemo(figure);
	}

	render(): void {
		renderDemo(this);
	}

	drawDemo(figure: HTMLElement, mouseLocation?: [number, number]): void {
		figure.innerHTML = this.callback(this.bezier, this.sliderData, mouseLocation, this.tVariant);
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
		return (Array.isArray(sliderUnit) ? sliderUnit[sliderValue] : sliderUnit) || "";
	}
}

export default BezierDemo;
