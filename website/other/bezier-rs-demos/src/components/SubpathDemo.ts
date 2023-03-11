import { WasmSubpath } from "@/../wasm/pkg";
import subpathFeatures, { SubpathFeatureKey } from "@/features/subpath-features";
import { renderDemo } from "@/utils/render";

import { SubpathCallback, WasmSubpathInstance, WasmSubpathManipulatorKey, SliderOption, TVariant } from "@/utils/types";

const SELECTABLE_RANGE = 10;
const POINT_INDEX_TO_MANIPULATOR: WasmSubpathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

class SubpathDemo extends HTMLElement {
	// Props
	title!: string;

	triples!: (number[] | undefined)[][];

	key!: SubpathFeatureKey;

	closed!: boolean;

	sliderOptions!: SliderOption[];

	triggerOnMouseMove!: boolean;

	tVariant!: TVariant;

	// Data
	subpath!: WasmSubpath;

	callback!: SubpathCallback;

	manipulatorKeys!: WasmSubpathManipulatorKey[];

	activeIndex!: number[] | undefined;

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
		this.triples = JSON.parse(this.getAttribute("triples") || "[]");
		this.key = this.getAttribute("key") as SubpathFeatureKey;
		this.sliderOptions = JSON.parse(this.getAttribute("sliderOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.closed = this.getAttribute("closed") === "true";
		this.tVariant = (this.getAttribute("tvariant") || "Parametric") as TVariant;

		this.callback = subpathFeatures[this.key].callback as SubpathCallback;
		this.sliderData = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.default })));
		this.sliderUnits = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.unit })));
		this.render();

		const figure = this.querySelector("figure") as HTMLElement;
		const wasm = await import("@/../wasm/pkg");
		this.subpath = wasm.WasmSubpath.from_triples(this.triples, this.closed) as WasmSubpathInstance;
		this.drawDemo(figure);
	}

	render(): void {
		renderDemo(this);
	}

	drawDemo(figure: HTMLElement, mouseLocation?: [number, number]): void {
		figure.innerHTML = this.callback(this.subpath, this.sliderData, mouseLocation, this.tVariant);
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

export default SubpathDemo;
