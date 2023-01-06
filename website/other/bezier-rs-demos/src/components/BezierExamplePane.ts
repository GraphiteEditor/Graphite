// eslint-disable-next-line no-restricted-imports, import/extensions, @typescript-eslint/no-unused-vars
import BezierExample from "./BezierExample";

import { BezierFeature } from "@/features/bezierFeatures"
import { BezierCurveType, BEZIER_CURVE_TYPE, ComputeType, ExampleOptions, SliderOption } from "@/utils/types";

const exampleDefaults = {
	Linear: {
		points: [
			[30, 60],
			[140, 120],
		],
	},
	Quadratic: {
		points: [
			[30, 50],
			[140, 30],
			[160, 170],
		],
	},
	Cubic: {
		points: [
			[30, 30],
			[60, 140],
			[150, 30],
			[160, 160],
		],
	},
};

type Example = {
	title: BezierCurveType,
	disabled: boolean,
	points: number[][],
	sliderOptions: SliderOption[],
}
		
class BezierExamplePane extends HTMLElement {
	// Props
	name!: BezierFeature;

	exampleOptions!: ExampleOptions;

	triggerOnMouseMove!: boolean;

	chooseComputeType!: boolean;

	// Data
	examples!: Example[];

	id: string = `${Math.random()}`.substring(2);

	computeType: ComputeType = "Parametric";

	connectedCallback(): void {
		this.name = (this.getAttribute("name") || "") as BezierFeature;
		this.exampleOptions = JSON.parse(this.getAttribute("exampleOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.chooseComputeType = this.getAttribute("chooseComputeType") === "true";
		
		// Use quadratic slider options as a default if sliders are not provided for the other curve types.
		const defaultSliderOptions: SliderOption[] = this.exampleOptions.Quadratic?.sliderOptions || [];

		this.examples = BEZIER_CURVE_TYPE.map((curveType: BezierCurveType) => {
			const givenData = this.exampleOptions[curveType];
			const defaultData = exampleDefaults[curveType];
			return {
				title: curveType,
				disabled: givenData?.disabled || false,
				points: givenData?.customPoints || defaultData.points,
				sliderOptions: givenData?.sliderOptions || defaultSliderOptions,
			};
		});
	}

	render(): void {
		const container = document.createElement("div");
		container.className = "example-pane-container";
		
		const header = document.createElement("h3");
		header.innerText = this.name;
		header.className = "example-pane-header";

		const computeTypeContainer = document.createElement("div");
		computeTypeContainer.className="compute-type-choice";

		const computeTypeLabel = document.createElement("strong");
		computeTypeLabel.innerText = "ComputeType:";
		computeTypeContainer.append(computeTypeLabel);

		["Parametric", "Euclidean"].forEach((computeType) => {
			const id = `${this.id}-${computeType}`
			const radioInput = document.createElement("input");
			radioInput.type ="radio";
			radioInput.id = id;
			radioInput.value = computeType;
			computeTypeContainer.append(radioInput);

			const label = document.createElement("label");
			label.htmlFor = id;
			label.innerText = computeType;
			computeTypeContainer.append(radioInput);
		})

		const exampleRow = document.createElement("div");
				exampleRow.className = "example-row";

		this.examples.forEach((example) => {
			if (example.disabled) {
				return;
			}
			const bezierExample = document.createElement("bezier-example");
			bezierExample.title = example.title;
			bezierExample.points = example.points; // JSON.stringify(example.points)
			bezierExample.name = this.name;
			bezierExample.sliderOptions = example.sliderOptions
			bezierExample.triggerOnMouseMove = this.triggerOnMouseMove
			bezierExample.setAttribute("computetype", this.computeType);

			exampleRow.append(bezierExample);
		})

		container.append(header);
		container.append(computeTypeContainer);
		container.append(exampleRow);
	}
}

window.customElements.define("bezier-example-pane", BezierExamplePane);

export default BezierExamplePane;
