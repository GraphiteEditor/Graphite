import { WasmBezier } from "@/../wasm/pkg";

import bezierFeatures from "@/features/bezierFeatures";
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

    name!: keyof typeof bezierFeatures;
    
    sliderOptions!: SliderOption[];
    
    triggerOnMouseMove!: boolean;

    computeTypeChoice!: ComputeType;

    callback!: BezierCallback;

    // Data
    bezier!: WasmBezier;

    manipulatorKeys!: WasmBezierManipulatorKey[];

    activeIndex!: number | undefined;

    mutablePoints!: number[][];

    sliderData: any;
    
    sliderUnits?: string | string[];

    static get observedAttributes(): string[] {
        return ["title", "points", "name", "sliderOptions", "triggerOnMouseMove", "computeType"];
    }

    connectedCallback(): void {
        this.title = this.getAttribute("title") || "";
        this.points = JSON.parse(this.getAttribute("points") || "")
        this.name = this.getAttribute("name") as keyof typeof bezierFeatures;
        this.sliderOptions = JSON.parse(this.getAttribute("sliderOptions") || "")
        this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
        this.computeTypeChoice = (this.getAttribute("computeTypeChoice") || "Parametric") as ComputeType;
        
        this.callback = bezierFeatures[this.name].callback as BezierCallback
        const curveType = getCurveType(this.points.length);
        
		this.manipulatorKeys = MANIPULATOR_KEYS_FROM_BEZIER_TYPE[curveType];
		this.bezier = WasmBezier[getConstructorKey(curveType)](this.points);
		this.sliderData = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.default })));
        this.activeIndex = undefined as number | undefined;
        this.mutablePoints = JSON.parse(JSON.stringify(this.points));
        this.sliderUnits = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.unit })));

        this.onMouseDown.bind(this);
        this.onMouseUp.bind(this);
        this.onMouseMove.bind(this);
        this.render();

        const figure = this.querySelector("figure") as HTMLElement;
        figure.innerHTML = this.callback(this.bezier, this.sliderData, undefined, this.computeTypeChoice)
    }

    //   attributeChangedCallback(name, oldValue, newValue) {
    //       this.render();
    //   }

    render(): void {
        const header = document.createElement("h4");
        header.className = "example-header";
        header.innerText = this.title;
        
        const figure = document.createElement("figure");
        figure.className = "example-figure"
        figure.addEventListener("mousedown", this.onMouseDown.bind(this))
        figure.addEventListener("mouseup", this.onMouseUp.bind(this))
        figure.addEventListener("mousemove", this.onMouseMove.bind(this))
        
        this.append(header);
        this.append(figure);
    }
    
    onMouseDown(event: MouseEvent): void {
        const mx = event.offsetX;
        const my = event.offsetY;
        for (let pointIndex = 0; pointIndex < this.points.length; pointIndex += 1) {
            const point = this.mutablePoints[pointIndex];
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
            this.mutablePoints[this.activeIndex] = [mx, my];
            figure.innerHTML = this.callback(this.bezier, this.sliderData, undefined, this.computeTypeChoice);
        } else if (this.triggerOnMouseMove) {
            figure.innerHTML = this.callback(this.bezier, this.sliderData, [mx, my], this.computeTypeChoice)
        }
    }

    static getSliderValue (sliderValue: number, sliderUnit?: string | string[]): string | undefined {
        return Array.isArray(sliderUnit) ? sliderUnit[sliderValue] : sliderUnit
    }
}

window.customElements.define("bezier-example", BezierExample);

export default BezierExample;
