import { BezierFeatureName } from "@/features/bezierFeatures";
import { renderExamplePane } from "@/utils/render";
import { BezierCurveType, BEZIER_CURVE_TYPE, ComputeType, BezierExampleOptions, SliderOption, Example, ExamplePane, BezierExampleArgs } from "@/utils/types";

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

class BezierExamplePane extends HTMLElement implements ExamplePane {
	// Props
	name!: BezierFeatureName;

	exampleOptions!: BezierExampleOptions;

	triggerOnMouseMove!: boolean;

	chooseComputeType!: boolean;

	// Data
	examples!: BezierExampleArgs[];

	id!: string;

	computeType!: ComputeType;

	connectedCallback(): void {
		this.id = `${Math.random()}`.substring(2);
		this.computeType = "Parametric";

		this.name = (this.getAttribute("name") || "") as BezierFeatureName;
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
		this.render();
	}

	render(): void {
		renderExamplePane(this);
	}

	buildExample(example: BezierExampleArgs): Example {
		const bezierExample = document.createElement("bezier-example");
		bezierExample.setAttribute("title", example.title);
		bezierExample.setAttribute("points", JSON.stringify(example.points));
		bezierExample.setAttribute("name", this.name);
		bezierExample.setAttribute("sliderOptions", JSON.stringify(example.sliderOptions));
		bezierExample.setAttribute("triggerOnMouseMove", String(this.triggerOnMouseMove));
		bezierExample.setAttribute("computetype", this.computeType);
		return bezierExample;
	}
}

export default BezierExamplePane;
