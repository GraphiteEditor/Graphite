import bezierFeatures, { BezierFeatureKey } from "@/features/bezier-features";
import { renderDemoPane } from "@/utils/render";
import { BezierCurveType, BEZIER_CURVE_TYPE, TVariant, BezierDemoOptions, SliderOption, Demo, DemoPane, BezierDemoArgs } from "@/utils/types";

const demoDefaults = {
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

class BezierDemoPane extends HTMLElement implements DemoPane {
	// Props
	key!: BezierFeatureKey;

	name!: string;

	demoOptions!: BezierDemoOptions;

	triggerOnMouseMove!: boolean;

	chooseTVariant!: boolean;

	// Data
	demos!: BezierDemoArgs[];

	id!: string;

	tVariant!: TVariant;

	connectedCallback(): void {
		this.tVariant = "Parametric";
		this.key = (this.getAttribute("name") || "") as BezierFeatureKey;
		this.id = `bezier/${this.key}`;
		this.name = bezierFeatures[this.key].name;
		this.demoOptions = JSON.parse(this.getAttribute("demoOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.chooseTVariant = this.getAttribute("chooseTVariant") === "true";
		// Use quadratic slider options as a default if sliders are not provided for the other curve types.
		const defaultSliderOptions: SliderOption[] = this.demoOptions.Quadratic?.sliderOptions || [];
		this.demos = BEZIER_CURVE_TYPE.map((curveType: BezierCurveType) => {
			const givenData = this.demoOptions[curveType];
			const defaultData = demoDefaults[curveType];
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
		renderDemoPane(this);
	}

	buildDemo(demo: BezierDemoArgs): Demo {
		const bezierDemo = document.createElement("bezier-demo");
		bezierDemo.setAttribute("title", demo.title);
		bezierDemo.setAttribute("points", JSON.stringify(demo.points));
		bezierDemo.setAttribute("key", this.key);
		bezierDemo.setAttribute("sliderOptions", JSON.stringify(demo.sliderOptions));
		bezierDemo.setAttribute("triggerOnMouseMove", String(this.triggerOnMouseMove));
		bezierDemo.setAttribute("tvariant", this.tVariant);
		return bezierDemo;
	}
}

export default BezierDemoPane;
