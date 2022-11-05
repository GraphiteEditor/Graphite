<template>
	<FieldInput
		class="number-input"
		:class="mode.toLocaleLowerCase()"
		v-model:value="text"
		:label="label"
		:spellcheck="false"
		:disabled="disabled"
		:style="{ 'min-width': minWidth > 0 ? `${minWidth}px` : undefined, '--travel-factor': rangeSliderValueBeforeMousedown / (sliderMinValue + sliderMaxValue) }"
		:tooltip="tooltip"
		:sharpRightCorners="sharpRightCorners"
		@textFocused="() => onTextFocused()"
		@textChanged="() => onTextChanged()"
		@cancelTextChange="() => onCancelTextChange()"
		ref="fieldInput"
	>
		<button v-if="value !== undefined && mode === 'Increment' && incrementBehavior !== 'None'" class="arrow left" @click="() => onIncrement('Decrease')" tabindex="-1"></button>
		<button v-if="value !== undefined && mode === 'Increment' && incrementBehavior !== 'None'" class="arrow right" @click="() => onIncrement('Increase')" tabindex="-1"></button>
		<input
			v-if="mode === 'Range' && value !== undefined"
			type="range"
			class="slider"
			:class="{ hidden: fakeSliderState === 'mousedown' }"
			v-model="rangeSliderValue"
			:min="sliderMinValue"
			:max="sliderMaxValue"
			:step="sliderStepValue"
			:disabled="disabled"
			@pointerdown="() => sliderPointerDown()"
			@pointerup="() => sliderPointerUp()"
			@input="() => sliderInput()"
		/>
		<div v-if="value !== undefined && fakeSliderState === 'mousedown'" class="fake-slider-thumb"></div>
		<div v-if="value !== undefined" class="slider-progress"></div>
	</FieldInput>
</template>

<style lang="scss">
.number-input {
	&.increment {
		// Widen the label and input margins from the edges by an extra 8px to make room for the increment arrows
		label {
			margin-left: 16px;
		}

		input:not(:focus).has-label {
			margin-right: 16px;
		}

		// Hide the increment arrows when focused, disabled, or not hovered
		input:focus ~ .arrow,
		&.disabled .arrow,
		&:not(:hover) .arrow {
			display: none;
		}

		// Style the increment arrows
		.arrow {
			position: absolute;
			top: 0;
			padding: 9px 0;
			border: none;
			background: rgba(var(--color-1-nearblack-rgb), 0.75);

			&:hover {
				background: var(--color-6-lowergray);

				&.right::before {
					border-color: transparent transparent transparent var(--color-f-white);
				}

				&.left::after {
					border-color: transparent var(--color-f-white) transparent transparent;
				}
			}

			&.right {
				right: 0;
				padding-left: 7px;
				padding-right: 6px;

				&::before {
					content: "";
					display: block;
					width: 0;
					height: 0;
					border-style: solid;
					border-width: 3px 0 3px 3px;
					border-color: transparent transparent transparent var(--color-e-nearwhite);
				}
			}

			&.left {
				left: 0;
				padding-left: 6px;
				padding-right: 7px;

				&::after {
					content: "";
					display: block;
					width: 0;
					height: 0;
					border-style: solid;
					border-width: 3px 3px 3px 0;
					border-color: transparent var(--color-e-nearwhite) transparent transparent;
				}
			}
		}
	}

	&.range {
		position: relative;

		.input,
		label {
			z-index: 1;
		}

		.input:focus ~ .slider,
		.input:focus ~ .fake-slider-thumb,
		.input:focus ~ .slider-progress {
			display: none;
		}

		.slider {
			position: absolute;
			left: 0;
			top: 0;
			width: 100%;
			height: 100%;
			padding: 0;
			margin: 0;
			-webkit-appearance: none; // TODO: Prefix necessary? Test on Safari
			appearance: none;
			background: none;
			cursor: default;
			// Except when disabled, the range slider goes above the label and input so it's interactable.
			// Then we use the blend mode to make it appear behind which works since the text is almost white and background almost black.
			// When disabled, the blend mode trick doesn't work with the grayer colors, but we don't need it to be interactable so it can actually go behind.
			z-index: 2;
			mix-blend-mode: screen;

			&.hidden {
				opacity: 0;
			}

			// Chromium and Safari
			&::-webkit-slider-thumb {
				-webkit-appearance: none; // TODO: Prefix necessary? Test on Safari
				appearance: none;
				border-radius: 2px;
				width: 4px;
				height: 24px;
				background: #494949; // Becomes var(--color-5-dullgray) with screen blend mode over var(--color-1-nearblack) background
			}

			&:hover::-webkit-slider-thumb {
				background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
			}

			&:disabled {
				mix-blend-mode: normal;
				z-index: 0;

				&::-webkit-slider-thumb {
					background: var(--color-4-dimgray);
				}
			}

			// Firefox
			// TODO: Fix Firefox not working
			&::-moz-range-thumb {
				border: none;
				border-radius: 2px;
				width: 4px;
				height: 24px;
				background: #494949; // Becomes var(--color-5-dullgray) with screen blend mode over var(--color-1-nearblack) background
			}

			&:hover::-moz-range-thumb {
				background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
			}

			&:hover ~ .slider-progress::before {
				background: var(--color-3-darkgray);
			}

			&::-moz-range-track {
				height: 0;
			}
		}

		// This fake slider thumb stays in the location of the real thumb while we have to hide the real slider between mousedown and mouseup or mousemove.
		// That's because the range input element moves to the pressed location immediately upon mousedown, but we don't want to show that yet.
		// Instead, we want to wait until the user does something:
		// Releasing the mouse means we reset the slider to its previous location, thus canceling the slider move. In that case, we focus the text entry.
		// Moving the mouse means we have begun dragging, so then we hide this fake one and continue showing the actual drag of the real slider.
		.fake-slider-thumb {
			position: absolute;
			left: 2px;
			right: 2px;
			top: 0;
			bottom: 0;
			z-index: 2;
			mix-blend-mode: screen;
			pointer-events: none;

			&::before {
				content: "";
				position: absolute;
				border-radius: 2px;
				margin-left: -2px;
				left: calc(var(--travel-factor) * 100%);
				width: 4px;
				height: 24px;
				background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
			}
		}

		.slider-progress {
			position: absolute;
			top: 2px;
			bottom: 2px;
			left: 2px;
			right: 2px;
			pointer-events: none;

			&::before {
				content: "";
				position: absolute;
				top: 0;
				left: 0;
				width: calc(var(--travel-factor) * 100% - 2px);
				height: 100%;
				background: var(--color-2-mildblack);
				border-radius: 1px 0 0 1px;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type NumberInputMode, type NumberInputIncrementBehavior } from "@/wasm-communication/messages";

import FieldInput from "@/components/widgets/inputs/FieldInput.vue";

export default defineComponent({
	emits: ["update:value"],
	props: {
		// Label
		label: { type: String as PropType<string>, required: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },

		// Disabled
		disabled: { type: Boolean as PropType<boolean>, default: false },

		// Value
		value: { type: Number as PropType<number>, required: false }, // When not provided, a dash is displayed
		min: { type: Number as PropType<number>, required: false },
		max: { type: Number as PropType<number>, required: false },
		isInteger: { type: Boolean as PropType<boolean>, default: false },

		// Number presentation
		displayDecimalPlaces: { type: Number as PropType<number>, default: 3 },
		unit: { type: String as PropType<string>, default: "" },
		unitIsHiddenWhenEditing: { type: Boolean as PropType<boolean>, default: true },

		// Mode behavior
		// "Increment" shows arrows and allows dragging left/right to change the value.
		// "Range" shows a range slider between some minimum and maximum value.
		mode: { type: String as PropType<NumberInputMode>, default: "Increment" },
		// When `mode` is "Increment", `step` is the multiplier or addend used with `incrementBehavior`.
		// When `mode` is "Range", `step` is the range slider's snapping increment if `isInteger` is `true`.
		step: { type: Number as PropType<number>, default: 1 },
		// `incrementBehavior` is only applicable with a `mode` of "Increment".
		// "Add"/"Multiply": The value is added or multiplied by `step`.
		// "None": the increment arrows are not shown.
		// "Callback": the functions `incrementCallbackIncrease` and `incrementCallbackDecrease` call custom behavior.
		incrementBehavior: { type: String as PropType<NumberInputIncrementBehavior>, default: "Add" },

		// Styling
		minWidth: { type: Number as PropType<number>, default: 0 },
		sharpRightCorners: { type: Boolean as PropType<boolean>, default: false },

		// Callbacks
		incrementCallbackIncrease: { type: Function as PropType<() => void>, required: false },
		incrementCallbackDecrease: { type: Function as PropType<() => void>, required: false },
	},
	data() {
		return {
			text: this.displayText(this.value),
			editing: false,
			rangeSliderValue: this.value !== undefined ? this.value : 0,
			rangeSliderValueBeforeMousedown: this.value !== undefined ? this.value : 0, // TODO: rename now that it's also after mousedown
			// "default": nothing is happening (default/reset state)
			// "mousedown": the user has pressed down the mouse but might now drag, or release,
			// so we show the fake slider at the old position in place of the real one which is still being dragged while hidden
			// "dragging": the user is dragging the slider around so we don't show the fake slider anymore
			fakeSliderThumbTravel: 0,
			fakeSliderState: "default" as "default" | "mousedown" | "dragging",
		};
	},
	computed: {
		sliderMinValue() {
			return this.min === undefined ? 0 : this.min;
		},
		sliderMaxValue() {
			return this.max === undefined ? 100 : this.max;
		},
		sliderStepValue() {
			const step = this.step === undefined ? 1 : this.step;
			return this.isInteger ? step : "any";
		},
	},
	methods: {
		sliderInput() {
			if (this.fakeSliderState === "default") {
				this.fakeSliderState = "mousedown";
				return;
			}

			if (this.fakeSliderState === "mousedown") {
				this.fakeSliderState = "dragging";
			}

			// Leaves 4 digits after the decimal point
			const exponent = 10 ** 4;
			const roundedValue = Math.round(this.rangeSliderValue * exponent) / exponent;

			this.rangeSliderValueBeforeMousedown = roundedValue;
			this.updateValue(roundedValue);
		},
		async sliderPointerDown() {
			this.rangeSliderValueBeforeMousedown = this.rangeSliderValue;
		},
		sliderPointerUp() {
			// User clicked but didn't drag, so we focus the text input element
			if (this.fakeSliderState === "mousedown") {
				const fieldInput = this.$refs.fieldInput as typeof FieldInput | undefined;
				const inputElement = fieldInput?.$el.querySelector("[data-input-element]") as HTMLInputElement | undefined;
				if (!inputElement) return;

				this.rangeSliderValue = this.rangeSliderValueBeforeMousedown;
				inputElement.focus();
			}

			this.fakeSliderState = "default";
		},
		onTextFocused() {
			if (this.value === undefined) this.text = "";
			else if (this.unitIsHiddenWhenEditing) this.text = `${this.value}`;
			else this.text = `${this.value}${unPluralize(this.unit, this.value)}`;

			this.editing = true;

			(this.$refs.fieldInput as typeof FieldInput | undefined)?.selectAllText(this.text);
		},
		// Called only when `value` is changed from the <input> element via user input and committed, either with the
		// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding)
		onTextChanged() {
			// The `unFocus()` call at the bottom of this function and in `onCancelTextChange()` causes this function to be run again, so this check skips a second run
			if (!this.editing) return;

			const parsed = parseFloat(this.text);
			const newValue = Number.isNaN(parsed) ? undefined : parsed;

			this.updateValue(newValue);

			this.editing = false;

			(this.$refs.fieldInput as typeof FieldInput | undefined)?.unFocus();
		},
		onCancelTextChange() {
			this.updateValue(undefined);

			this.editing = false;

			(this.$refs.fieldInput as typeof FieldInput | undefined)?.unFocus();
		},
		onIncrement(direction: "Decrease" | "Increase") {
			if (this.value === undefined) return;

			const actions = {
				Add: (): void => {
					const directionAddend = direction === "Increase" ? this.step : -this.step;
					this.updateValue(this.value !== undefined ? this.value + directionAddend : undefined);
				},
				Multiply: (): void => {
					const directionMultiplier = direction === "Increase" ? this.step : 1 / this.step;
					this.updateValue(this.value !== undefined ? this.value * directionMultiplier : undefined);
				},
				Callback: (): void => {
					if (direction === "Increase") this.incrementCallbackIncrease?.();
					if (direction === "Decrease") this.incrementCallbackDecrease?.();
				},
				None: (): void => undefined,
			};
			const action = actions[this.incrementBehavior];
			action();
		},
		updateValue(newValue: number | undefined) {
			const nowValid = this.value !== undefined && this.isInteger ? Math.round(this.value) : this.value;
			let cleaned = newValue !== undefined ? newValue : nowValid;

			if (typeof this.min === "number" && !Number.isNaN(this.min) && cleaned !== undefined) cleaned = Math.max(cleaned, this.min);
			if (typeof this.max === "number" && !Number.isNaN(this.max) && cleaned !== undefined) cleaned = Math.min(cleaned, this.max);

			// Required as the call to update:value can, not change the value
			this.text = this.displayText(this.value);

			if (newValue !== undefined) this.$emit("update:value", cleaned);
		},
		displayText(value: number | undefined): string {
			if (value === undefined) return "-";

			// Find the amount of digits on the left side of the decimal
			// 10.25 == 2
			// 1.23 == 1
			// 0.23 == 0 (Reason for the slightly more complicated code)
			const absValueInt = Math.floor(Math.abs(value));
			const leftSideDigits = absValueInt === 0 ? 0 : absValueInt.toString().length;
			const roundingPower = 10 ** Math.max(this.displayDecimalPlaces - leftSideDigits, 0);

			const displayValue = Math.round(value * roundingPower) / roundingPower;

			return `${displayValue}${unPluralize(this.unit, value)}`;
		},
	},
	watch: {
		// Called only when `value` is changed from outside this component (with v-model)
		value(newValue: number | undefined) {
			if (newValue === undefined) {
				this.text = "-";
				return;
			}

			this.rangeSliderValue = newValue;
			this.rangeSliderValueBeforeMousedown = newValue;

			// The simple `clamp()` function can't be used here since `undefined` values need to be boundless
			let sanitized = newValue;
			if (typeof this.min === "number") sanitized = Math.max(sanitized, this.min);
			if (typeof this.max === "number") sanitized = Math.min(sanitized, this.max);

			this.text = this.displayText(sanitized);
		},
	},
	components: { FieldInput },
});

function unPluralize(unit: string, value: number): string {
	if (value === 1 && unit.endsWith("s")) return unit.slice(0, -1);
	return unit;
}
</script>
