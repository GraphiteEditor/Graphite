import { WasmSubpath } from "@/../wasm/pkg";
import subpathFeatures, { SubpathFeature } from "@/features/subpathFeatures";
import { renderExample } from "@/utils/render";

import { SubpathCallback, WasmSubpathInstance, WasmSubpathManipulatorKey, SliderOption, ComputeType } from "@/utils/types";

const SELECTABLE_RANGE = 10;
const POINT_INDEX_TO_MANIPULATOR: WasmSubpathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

class SubpathExample extends HTMLElement {
	// Props
	title!: string;

	triples!: (number[] | undefined)[][];

	name!: SubpathFeature;

	closed!: boolean;

	sliderOptions!: SliderOption[];

	triggerOnMouseMove!: boolean;

	computeType!: ComputeType;

	// Data
	subpath!: WasmSubpath;

	callback!: SubpathCallback;

	manipulatorKeys!: WasmSubpathManipulatorKey[];

	activeIndex!: number[] | undefined;

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
		this.triples = JSON.parse(this.getAttribute("triples") || "[]");
		this.name = this.getAttribute("name") as SubpathFeature;
		this.sliderOptions = JSON.parse(this.getAttribute("sliderOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.closed = this.getAttribute("closed") === "true";
		this.computeType = (this.getAttribute("computetype") || "Parametric") as ComputeType;

		this.callback = subpathFeatures[this.name].callback as SubpathCallback;
		this.subpath = WasmSubpath.from_triples(this.triples, this.closed) as WasmSubpathInstance;
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
		figure.innerHTML = this.callback(this.subpath, this.sliderData, mouseLocation, this.computeType);
	}

	onMouseDown(event: MouseEvent): void {
		const mx = event.offsetX;
		const my = event.offsetY;
		for (let controllerIndex = 0; controllerIndex < this.triples.length; controllerIndex += 1) {
			for (let pointIndex = 0; pointIndex < 3; pointIndex += 1) {
				const point = this.triples[controllerIndex][pointIndex];
				if (point && Math.abs(mx - point[0]) < SELECTABLE_RANGE && Math.abs(my - point[1]) < SELECTABLE_RANGE) {
					this.activeIndex = [controllerIndex, pointIndex];
					return;
				}
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
		if (this.activeIndex) {
			this.subpath[POINT_INDEX_TO_MANIPULATOR[this.activeIndex[1]]](this.activeIndex[0], mx, my);
			this.triples[this.activeIndex[0]][this.activeIndex[1]] = [mx, my];
			this.drawExample(figure);
		} else if (this.triggerOnMouseMove) {
			this.drawExample(figure, [mx, my]);
		}
	}

	getSliderUnit(sliderValue: number, variable: string): string {
		const sliderUnit = this.sliderUnits[variable]
		return (Array.isArray(sliderUnit) ? sliderUnit[sliderValue] : sliderUnit) || "";
	}
}

export default SubpathExample;
