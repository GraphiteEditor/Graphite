import subpathFeatures, { SubpathFeatureKey } from "@/features/subpath-features";
import { renderDemoPane } from "@/utils/render";
import { ComputeType, Demo, DemoPane, SliderOption, SubpathDemoArgs } from "@/utils/types";

class SubpathDemoPane extends HTMLElement implements DemoPane {
	// Props
	key!: SubpathFeatureKey;

	name!: string;

	sliderOptions!: SliderOption[];

	triggerOnMouseMove!: boolean;

	chooseComputeType!: boolean;

	// Data
	demos!: SubpathDemoArgs[];

	id!: string;

	computeType!: ComputeType;

	connectedCallback(): void {
		this.demos = [
			{
				title: "Open Subpath",
				triples: [
					[[20, 20], undefined, [10, 90]],
					[[150, 40], [60, 40], undefined],
					[[175, 175], undefined, undefined],
					[[100, 100], [40, 120], undefined],
				],
				closed: false,
			},
			{
				title: "Closed Subpath",
				triples: [
					[[35, 125], undefined, [40, 40]],
					[[130, 30], [120, 120], undefined],
					[
						[145, 150],
						[175, 90],
						[70, 185],
					],
				],
				closed: true,
			},
		];
		this.id = `${Math.random()}`.substring(2);
		this.computeType = "Parametric";

		this.key = (this.getAttribute("name") || "") as SubpathFeatureKey;
		this.name = subpathFeatures[this.key].name;
		this.sliderOptions = JSON.parse(this.getAttribute("sliderOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.chooseComputeType = this.getAttribute("chooseComputeType") === "true";

		this.render();
	}

	render(): void {
		renderDemoPane(this);
	}

	buildDemo(demo: SubpathDemoArgs): Demo {
		const subpathDemo = document.createElement("subpath-demo");
		subpathDemo.setAttribute("title", demo.title);
		subpathDemo.setAttribute("triples", JSON.stringify(demo.triples));
		subpathDemo.setAttribute("closed", String(demo.closed));
		subpathDemo.setAttribute("key", this.key);
		subpathDemo.setAttribute("sliderOptions", JSON.stringify(this.sliderOptions));
		subpathDemo.setAttribute("triggerOnMouseMove", String(this.triggerOnMouseMove));
		subpathDemo.setAttribute("computetype", this.computeType);
		return subpathDemo;
	}
}

export default SubpathDemoPane;
