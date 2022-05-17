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
		<button v-if="value !== undefined" class="arrow left" @click="() => onIncrement('Decrease')"></button>
		<button v-if="value !== undefined" class="arrow right" @click="() => onIncrement('Increase')"></button>
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
		value: { type: Number as PropType<number>, required: false }, // When not provided, a dash is displayed
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
			if (this.value === undefined) this.text = "";
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
			if (!this.editing) return;

			const parsed = parseFloat(this.text);
			const newValue = Number.isNaN(parsed) ? undefined : parsed;

			this.updateValue(newValue);

			this.editing = false;

			const inputElement = (this.$refs.fieldInput as typeof FieldInput).$refs.input as HTMLInputElement;
			inputElement.blur();
		},
		onCancelTextChange() {
			this.updateValue(undefined);

			this.editing = false;

			const inputElement = (this.$refs.fieldInput as typeof FieldInput).$refs.input as HTMLInputElement;
			inputElement.blur();
		},
		onIncrement(direction: IncrementDirection) {
			if (this.value === undefined) return;

			const actions = {
				Add: (): void => {
					const directionAddend = direction === "Increase" ? this.incrementFactor : -this.incrementFactor;
					this.updateValue(this.value !== undefined ? this.value + directionAddend : undefined);
				},
				Multiply: (): void => {
					const directionMultiplier = direction === "Increase" ? this.incrementFactor : 1 / this.incrementFactor;
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

			if (newValue !== undefined) this.$emit("update:value", cleaned);

			this.text = this.displayText(cleaned);
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

			return `${displayValue}${this.unit}`;
		},
	},
	watch: {
		// Called only when `value` is changed from outside this component (with v-model)
		value(newValue: number | undefined) {
			if (newValue === undefined) {
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
