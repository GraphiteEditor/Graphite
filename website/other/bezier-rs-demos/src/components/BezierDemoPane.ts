import bezierFeatures, { BezierFeatureKey } from "@/features/bezier-features";
import { renderDemoPane } from "@/utils/render";
import { BezierCurveType, BEZIER_CURVE_TYPE, BezierDemoOptions, InputOption, Demo, DemoPane, BezierDemoArgs } from "@/utils/types";

const demoDefaults = {
	Linear: {
		points: [
			[55, 60],
			[165, 120],
		],
	},
	Quadratic: {
		points: [
			[55, 50],
			[165, 30],
			[185, 170],
		],
	},
	Cubic: {
		points: [
			[55, 30],
			[85, 140],
			[175, 30],
			[185, 160],
		],
	},
};

class BezierDemoPane extends HTMLElement implements DemoPane {
	// Props
	key!: BezierFeatureKey;

	name!: string;

	demoOptions!: BezierDemoOptions;

	triggerOnMouseMove!: boolean;

	// Data
	demos!: BezierDemoArgs[];

	id!: string;

	connectedCallback() {
		this.key = (this.getAttribute("name") || "") as BezierFeatureKey;
		this.id = `bezier/${this.key}`;
		this.name = bezierFeatures[this.key].name;
		this.demoOptions = JSON.parse(this.getAttribute("demoOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		// Use quadratic slider options as a default if sliders are not provided for the other curve types.
		const defaultSliderOptions: InputOption[] = this.demoOptions.Quadratic?.inputOptions || [];
		this.demos = BEZIER_CURVE_TYPE.map((curveType: BezierCurveType) => {
			const givenData = this.demoOptions[curveType];
			const defaultData = demoDefaults[curveType];
			return {
				title: curveType,
				disabled: givenData?.disabled || false,
				points: givenData?.customPoints || defaultData.points,
				inputOptions: givenData?.inputOptions || defaultSliderOptions,
			};
		});
		this.render();
	}

	render() {
		renderDemoPane(this);
	}

	buildDemo(demo: BezierDemoArgs): Demo {
		const bezierDemo = document.createElement("bezier-demo");
		bezierDemo.setAttribute("title", demo.title);
		bezierDemo.setAttribute("points", JSON.stringify(demo.points));
		bezierDemo.setAttribute("key", this.key);
		bezierDemo.setAttribute("inputOptions", JSON.stringify(demo.inputOptions));
		bezierDemo.setAttribute("triggerOnMouseMove", String(this.triggerOnMouseMove));
		return bezierDemo;
	}
}

export default BezierDemoPane;
