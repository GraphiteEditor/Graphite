import { ComputeType, Example, ExamplePane, SliderOption } from "@/utils/types";

export function renderExample(example: Example): void {
	const header = document.createElement("h4");
	header.className = "example-header";
	header.innerText = example.title;

	const figure = document.createElement("figure");
	figure.className = "example-figure";
	figure.addEventListener("mousedown", example.onMouseDown.bind(example));
	figure.addEventListener("mouseup", example.onMouseUp.bind(example));
	figure.addEventListener("mousemove", example.onMouseMove.bind(example));

	example.append(header);
	example.append(figure);

	example.sliderOptions.forEach((sliderOption: SliderOption) => {
		const sliderLabel = document.createElement("div");
		const sliderData = example.sliderData[sliderOption.variable];
		const sliderUnit = example.getSliderUnit(sliderData, sliderOption.variable);
		sliderLabel.className = "slider-label";
		sliderLabel.innerText = `${sliderOption.variable} = ${sliderData}${sliderUnit}`;
		example.append(sliderLabel);

		const sliderInput = document.createElement("input");
		sliderInput.className = "slider-input";
		sliderInput.type = "range";
		sliderInput.max = String(sliderOption.max);
		sliderInput.min = String(sliderOption.min);
		sliderInput.step = String(sliderOption.step);
		sliderInput.value = String(sliderOption.default);
		sliderInput.addEventListener("input", (event: Event): void => {
			example.sliderData[sliderOption.variable] = Number((event.target as HTMLInputElement).value);
			sliderLabel.innerText = `${sliderOption.variable} = ${example.sliderData[sliderOption.variable]}${sliderUnit}`;
			example.drawExample(figure);
		});
		example.append(sliderInput);
	});
}

export function renderExamplePane(examplePane: ExamplePane): void {
	const container = document.createElement("div");
	container.className = "example-pane-container";

	const header = document.createElement("h3");
	header.innerText = examplePane.name;
	header.className = "example-pane-header";

	const computeTypeContainer = document.createElement("div");
	computeTypeContainer.className = "compute-type-choice";

	const computeTypeLabel = document.createElement("strong");
	computeTypeLabel.innerText = "ComputeType:";
	computeTypeContainer.append(computeTypeLabel);

	const radioInputs = ["Parametric", "Euclidean"].map((computeType) => {
		const id = `${examplePane.id}-${computeType}`;
		const radioInput = document.createElement("input");
		radioInput.type = "radio";
		radioInput.id = id;
		radioInput.value = computeType;
		radioInput.name = "ComputeType";
		radioInput.checked = computeType === "Parametric";
		computeTypeContainer.append(radioInput);

		const label = document.createElement("label");
		label.htmlFor = id;
		label.innerText = computeType;
		computeTypeContainer.append(label);
		return radioInput;
	});

	const exampleRow = document.createElement("div");
	exampleRow.className = "example-row";

	examplePane.examples.forEach((example) => {
		if (example.disabled) {
			return;
		}
		const exampleComponent = examplePane.buildExample(example);

		radioInputs.forEach((radioInput: HTMLElement) => {
			radioInput.addEventListener("input", (event: Event): void => {
				examplePane.computeType = (event.target as HTMLInputElement).value as ComputeType;
				exampleComponent.setAttribute("computetype", examplePane.computeType);
			});
		});
		exampleRow.append(exampleComponent);
	});

	container.append(header);
	if (examplePane.chooseComputeType) {
		container.append(computeTypeContainer);
	}
	container.append(exampleRow);

	examplePane.append(container);
}
