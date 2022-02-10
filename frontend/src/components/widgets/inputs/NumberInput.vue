<template>
	<FieldInput
		class="number-input"
		v-model:value="text"
		:label="label"
		:spellcheck="false"
		:disabled="disabled"
		@textFocused="() => onTextFocused()"
		@textChanged="() => onTextChanged()"
		@cancelTextChange="() => onCancelTextChange()"
		ref="fieldInput"
	>
		<button v-if="!Number.isNaN(value)" class="arrow left" @click="() => onIncrement('Decrease')"></button>
		<button v-if="!Number.isNaN(value)" class="arrow right" @click="() => onIncrement('Increase')"></button>
	</FieldInput>
</template>

<style lang="scss">
.number-input {
	input:focus ~ .arrow {
		display: none;
	}

	&:not(:hover) .arrow {
		display: none;
	}

	.arrow {
		position: absolute;
		top: 0;
		padding: 9px 0;
		outline: none;
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
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 0 3px 3px;
				border-color: transparent transparent transparent var(--color-e-nearwhite);
				display: block;
			}
		}

		&.left {
			left: 0;
			padding-left: 6px;
			padding-right: 7px;

			&::after {
				content: "";
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 3px 3px 0;
				border-color: transparent var(--color-e-nearwhite) transparent transparent;
				display: block;
			}
		}
	}

	&.disabled .arrow {
		display: none;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IncrementBehavior, IncrementDirection } from "@/utilities/widgets";

import FieldInput from "@/components/widgets/inputs/FieldInput.vue";

export default defineComponent({
	emits: ["update:value"],
	props: {
		value: { type: Number as PropType<number>, required: true },
		min: { type: Number as PropType<number>, required: false },
		max: { type: Number as PropType<number>, required: false },
		incrementBehavior: { type: String as PropType<IncrementBehavior>, default: "Add" },
		incrementFactor: { type: Number as PropType<number>, default: 1 },
		incrementCallbackIncrease: { type: Function as PropType<() => void>, required: false },
		incrementCallbackDecrease: { type: Function as PropType<() => void>, required: false },
		isInteger: { type: Boolean as PropType<boolean>, default: false },
		unit: { type: String as PropType<string>, default: "" },
		unitIsHiddenWhenEditing: { type: Boolean as PropType<boolean>, default: true },
		displayDecimalPlaces: { type: Number as PropType<number>, default: 3 },
		label: { type: String as PropType<string>, required: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			text: this.displayText(this.value),
			editing: false,
		};
	},
	methods: {
		onTextFocused() {
			if (Number.isNaN(this.value)) this.text = "";
			else if (this.unitIsHiddenWhenEditing) this.text = `${this.value}`;
			else this.text = `${this.value}${this.unit}`;

			this.editing = true;

			const inputElement = (this.$refs.fieldInput as typeof FieldInput).$refs.input as HTMLInputElement;
			// Setting the value directly is required to make `inputElement.select()` work
			inputElement.value = this.text;
			inputElement.select();
		},
		// Called only when `value` is changed from the <input> element via user input and committed, either with the
		// enter key (via the `change` event) or when the <input> element is defocused (with the `blur` event binding)
		onTextChanged() {
			// The `inputElement.blur()` call at the bottom of this function causes itself to be run again, so this check skips a second run
			if (this.editing) {
				const newValue = parseFloat(this.text);
				this.updateValue(newValue);

				this.editing = false;

				const inputElement = (this.$refs.fieldInput as typeof FieldInput).$refs.input as HTMLInputElement;
				inputElement.blur();
			}
		},
		onCancelTextChange() {
			this.updateValue(NaN);

			this.editing = false;

			const inputElement = (this.$refs.fieldInput as typeof FieldInput).$refs.input as HTMLInputElement;
			inputElement.blur();
		},
		onIncrement(direction: IncrementDirection) {
			if (Number.isNaN(this.value)) return;

			({
				Add: (): void => {
					const directionAddend = direction === "Increase" ? this.incrementFactor : -this.incrementFactor;
					this.updateValue(this.value + directionAddend);
				},
				Multiply: (): void => {
					const directionMultiplier = direction === "Increase" ? this.incrementFactor : 1 / this.incrementFactor;
					this.updateValue(this.value * directionMultiplier);
				},
				Callback: (): void => {
					if (direction === "Increase" && this.incrementCallbackIncrease) this.incrementCallbackIncrease();
					if (direction === "Decrease" && this.incrementCallbackDecrease) this.incrementCallbackDecrease();
				},
				None: (): void => undefined,
			}[this.incrementBehavior]());
		},
		updateValue(newValue: number) {
			const invalid = Number.isNaN(newValue);

			let sanitized = newValue;
			if (invalid) sanitized = this.value;
			if (this.isInteger) sanitized = Math.round(sanitized);
			if (typeof this.min === "number" && !Number.isNaN(this.min)) sanitized = Math.max(sanitized, this.min);
			if (typeof this.max === "number" && !Number.isNaN(this.max)) sanitized = Math.min(sanitized, this.max);

			if (!invalid) this.$emit("update:value", sanitized);

			this.text = this.displayText(sanitized);
		},
		displayText(value: number): string {
			// Find the amount of digits on the left side of the decimal
			// 10.25 == 2
			// 1.23 == 1
			// 0.23 == 0 (Reason for the slightly more complicated code)
			const leftSideDigits = Math.abs(Math.max(Math.floor(value).toString().length, 0) * Math.sign(value));
			const roundingPower = 10 ** Math.max(this.displayDecimalPlaces - leftSideDigits, 0);

			const displayValue = Math.round(value * roundingPower) / roundingPower;

			return `${displayValue}${this.unit}`;
		},
	},
	watch: {
		// Called only when `value` is changed from outside this component (with v-model)
		value(newValue: number) {
			if (Number.isNaN(newValue)) {
				this.text = "-";
				return;
			}

			// The simple `clamp()` function can't be used here since `undefined` values need to be boundless
			let sanitized = newValue;
			if (typeof this.min === "number") sanitized = Math.max(sanitized, this.min);
			if (typeof this.max === "number") sanitized = Math.min(sanitized, this.max);

			this.text = this.displayText(sanitized);
		},
	},
	components: { FieldInput },
});
</script>
