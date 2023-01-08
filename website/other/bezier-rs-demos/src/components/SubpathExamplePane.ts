import { SubpathFeatureName } from "@/features/subpathFeatures";
import { renderExamplePane } from "@/utils/render";
import { ComputeType, Example, ExamplePane, SliderOption, SubpathExampleArgs } from "@/utils/types";

class SubpathExamplePane extends HTMLElement implements ExamplePane {
	// Props
	name!: SubpathFeatureName;

	sliderOptions!: SliderOption[];

	triggerOnMouseMove!: boolean;

	chooseComputeType!: boolean;

	// Data
	examples!: SubpathExampleArgs[];

	id!: string;

	computeType!: ComputeType;

	connectedCallback(): void {
		this.examples = [
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

		this.name = (this.getAttribute("name") || "") as SubpathFeatureName;
		this.sliderOptions = JSON.parse(this.getAttribute("sliderOptions") || "[]");
		this.triggerOnMouseMove = this.getAttribute("triggerOnMouseMove") === "true";
		this.chooseComputeType = this.getAttribute("chooseComputeType") === "true";

		this.render();
	}

	render(): void {
		renderExamplePane(this);
	}

	buildExample(example: SubpathExampleArgs): Example {
		const subpathExample = document.createElement("subpath-example");
		subpathExample.setAttribute("title", example.title);
		subpathExample.setAttribute("triples", JSON.stringify(example.triples));
		subpathExample.setAttribute("closed", String(example.closed));
		subpathExample.setAttribute("name", this.name);
		subpathExample.setAttribute("sliderOptions", JSON.stringify(this.sliderOptions));
		subpathExample.setAttribute("triggerOnMouseMove", String(this.triggerOnMouseMove));
		subpathExample.setAttribute("computetype", this.computeType);
		return subpathExample;
	}
}

export default SubpathExamplePane;
