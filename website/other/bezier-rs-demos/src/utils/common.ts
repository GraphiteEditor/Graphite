import { Example, SliderOption } from "@/utils/types";

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
