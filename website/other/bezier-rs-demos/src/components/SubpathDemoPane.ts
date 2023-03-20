import subpathFeatures, { SubpathFeatureKey } from "@/features/subpath-features";
import { renderDemoPane } from "@/utils/render";
import { Demo, DemoPane, InputOption, SubpathDemoArgs } from "@/utils/types";

class SubpathDemoPane extends HTMLElement implements DemoPane {
	// Props
	key!: SubpathFeatureKey;

	name!: string;

	inputOptions!: InputOption[];

	triggerOnMouseMove!: boolean;

	// Data
	demos!: SubpathDemoArgs[];

	id!: string;

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
		this.key = (this.getAttribute("name") || "") as SubpathFeatureKey;
		this.id = `subpath/${this.key}`;
		this.name = subpathFeatures[this.key].name;
		this.inputOptions = JSON.parse(this.getAttribute("inputOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";

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
		subpathDemo.setAttribute("inputOptions", JSON.stringify(this.inputOptions));
		subpathDemo.setAttribute("triggerOnMouseMove", String(this.triggerOnMouseMove));
		return subpathDemo;
	}
}

export default SubpathDemoPane;
