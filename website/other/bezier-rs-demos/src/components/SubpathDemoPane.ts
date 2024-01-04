import type { SubpathFeatureKey } from "@/features/subpath-features";
import subpathFeatures from "@/features/subpath-features";
import { renderDemoPane } from "@/utils/render";
import type { DemoPane, SubpathDemoArgs, SubpathInputOption } from "@/utils/types";

class SubpathDemoPane extends HTMLElement implements DemoPane {
	// Props
	key!: SubpathFeatureKey;

	name!: string;

	inputOptions!: SubpathInputOption[];

	triggerOnMouseMove!: boolean;

	// Data
	demos!: SubpathDemoArgs[];

	id!: string;

	connectedCallback() {
		this.demos = [
			{
				title: "Open Subpath",
				triples: [
					[[45, 20], undefined, [35, 90]],
					[[175, 40], [85, 40], undefined],
					[[200, 175], undefined, undefined],
					[[125, 100], [65, 120], undefined],
				],
				closed: false,
			},
			{
				title: "Closed Subpath",
				triples: [
					[[60, 125], undefined, [65, 40]],
					[[155, 30], [145, 120], undefined],
					[
						[170, 150],
						[200, 90],
						[95, 185],
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

	render() {
		renderDemoPane(this);
	}

	buildDemo(demo: SubpathDemoArgs): HTMLElement {
		const subpathDemo = document.createElement("subpath-demo");
		subpathDemo.setAttribute("title", demo.title);
		subpathDemo.setAttribute("triples", JSON.stringify(demo.triples));
		subpathDemo.setAttribute("closed", String(demo.closed));
		subpathDemo.setAttribute("key", this.key);

		const inputOptions = this.inputOptions.map((option) => ({
			...option,
			disabled: option.isDisabledForClosed && demo.closed,
		}));
		subpathDemo.setAttribute("inputOptions", JSON.stringify(inputOptions));
		subpathDemo.setAttribute("triggerOnMouseMove", String(this.triggerOnMouseMove));
		return subpathDemo;
	}
}

export default SubpathDemoPane;
