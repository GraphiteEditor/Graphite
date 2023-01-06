import { WasmBezier } from "@/../wasm/pkg";

import bezierFeatures, { BezierFeature } from "@/features/bezierFeatures";
import { getConstructorKey, getCurveType, BezierCallback, BezierCurveType, SliderOption, WasmBezierManipulatorKey, ComputeType } from "@/utils/types";

const SELECTABLE_RANGE = 10;

// Given the number of points in the curve, map the index of a point to the correct manipulator key
const MANIPULATOR_KEYS_FROM_BEZIER_TYPE: { [key in BezierCurveType]: WasmBezierManipulatorKey[] } = {
	Linear: ["set_start", "set_end"],
	Quadratic: ["set_start", "set_handle_start", "set_end"],
	Cubic: ["set_start", "set_handle_start", "set_handle_end", "set_end"],
};

class BezierExample extends HTMLElement {
	// Props
	title!: string;

	points!: number[][];

	name!: BezierFeature;

	sliderOptions!: SliderOption[];

	triggerOnMouseMove!: boolean;

	computeType!: ComputeType;

	callback!: BezierCallback;

	// Data
	bezier!: WasmBezier;

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
			figure.innerHTML = this.callback(this.bezier, this.sliderData, undefined, this.computeType);
		}
	}

	connectedCallback(): void {
		this.title = this.getAttribute("title") || "";
		this.points = JSON.parse(this.getAttribute("points") || "[]");
		this.name = this.getAttribute("name") as keyof typeof bezierFeatures;
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

		this.onMouseDown.bind(this);
		this.onMouseUp.bind(this);
		this.onMouseMove.bind(this);
		this.render();

		const figure = this.querySelector("figure") as HTMLElement;
		figure.innerHTML = this.callback(this.bezier, this.sliderData, undefined, this.computeType);
	}

	render(): void {
		const header = document.createElement("h4");
		header.className = "example-header";
		header.innerText = this.title;

		const figure = document.createElement("figure");
		figure.className = "example-figure";
		figure.addEventListener("mousedown", this.onMouseDown.bind(this));
		figure.addEventListener("mouseup", this.onMouseUp.bind(this));
		figure.addEventListener("mousemove", this.onMouseMove.bind(this));

		this.append(header);
		this.append(figure);

		this.sliderOptions.forEach((sliderOption) => {
			const sliderLabel = document.createElement("div");
			const sliderData = this.sliderData[sliderOption.variable];
			const sliderUnit = BezierExample.getSliderUnit(sliderData, this.sliderUnits[sliderOption.variable]);
			sliderLabel.className = "slider-label";
			sliderLabel.innerText = `${sliderOption.variable} = ${sliderData}${sliderUnit}`;
			this.append(sliderLabel);

			const sliderInput = document.createElement("input");
			sliderInput.className = "slider-input";
			sliderInput.type = "range";
			sliderInput.max = String(sliderOption.max);
			sliderInput.min = String(sliderOption.min);
			sliderInput.step = String(sliderOption.step);
			sliderInput.value = String(sliderOption.default);
			sliderInput.addEventListener("input", (event: Event): void => {
				this.sliderData[sliderOption.variable] = Number((event.target as HTMLInputElement).value);
				sliderLabel.innerText = `${sliderOption.variable} = ${this.sliderData[sliderOption.variable]}${sliderUnit}`;
				figure.innerHTML = this.callback(this.bezier, this.sliderData, undefined, this.computeType);
			});
			this.append(sliderInput);
		});
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
			figure.innerHTML = this.callback(this.bezier, this.sliderData, undefined, this.computeType);
		} else if (this.triggerOnMouseMove) {
			figure.innerHTML = this.callback(this.bezier, this.sliderData, [mx, my], this.computeType);
		}
	}

	static getSliderUnit(sliderValue: number, sliderUnit?: string | string[]): string {
		return (Array.isArray(sliderUnit) ? sliderUnit[sliderValue] : sliderUnit) || "";
	}
}

window.customElements.define("bezier-example", BezierExample);

export default BezierExample;

declare global {   
	interface HTMLElementTagNameMap {
		"bezier-example": BezierExample;   
 } 
}
