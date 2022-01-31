<template>
	<LayoutRow class="number-input" :class="{ disabled }">
		<input
			:class="{ 'has-label': label }"
			:id="`number-input-${id}`"
			type="text"
			spellcheck="false"
			v-model="text"
			@change="onTextChanged()"
			@keydown.esc="onCancelTextChange"
			ref="input"
			:disabled="disabled"
		/>
		<label v-if="label" :for="`number-input-${id}`">{{ label }}</label>
		<button v-if="!Number.isNaN(value)" class="arrow left" @click="onIncrement('Decrease')"></button>
		<button v-if="!Number.isNaN(value)" class="arrow right" @click="onIncrement('Increase')"></button>
	</LayoutRow>
</template>

<style lang="scss">
.number-input {
	min-width: 80px;
	height: 24px;
	position: relative;
	border-radius: 2px;
	background: var(--color-1-nearblack);
	overflow: hidden;
	flex-direction: row-reverse;

	label {
		flex: 1 1 100%;
		line-height: 18px;
		margin-left: 8px;
		padding: 3px 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	&:not(.disabled) label {
		cursor: text;
	}

	input {
		flex: 1 1 100%;
		width: 0;
		min-width: 30px;
		height: 18px;
		line-height: 18px;
		margin: 0 8px;
		padding: 3px 0;
		outline: none;
		border: none;
		background: none;
		color: var(--color-e-nearwhite);
		text-align: center;

		&:not(:focus).has-label {
			text-align: right;
			margin-left: 0;
			margin-right: 8px;
		}

		&:focus {
			text-align: left;

			& + label,
			& ~ .arrow {
				display: none;
			}
		}
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

	&.disabled {
		background: var(--color-2-mildblack);

		label,
		input {
			color: var(--color-8-uppergray);
		}

		.arrow {
			display: none;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IncrementBehavior, IncrementDirection } from "@/utilities/widgets";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
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
			text: this.generateText(this.value),
			editing: false,
			id: `${Math.random()}`.substring(2),
		};
	},
	methods: {
		onTextFocused() {
			if (Number.isNaN(this.value)) this.text = "";
			else if (this.unitIsHiddenWhenEditing) this.text = `${this.value}`;
			else this.text = `${this.value}${this.unit}`;
			this.editing = true;
			const inputElement = this.$refs.input as HTMLInputElement;
			// Setting the value directly is required to make `inputElement.select()` work
			inputElement.value = this.text;
			inputElement.select();
		},
		// Called only when `value` is changed from the <input> element via user input and committed, either with the
		// enter key (via the `change` event) or when the <input> element is defocused (with the `blur` event binding)
		onTextChanged() {
			// The `inputElement.blur()` call at the bottom of this function causes itself to be run again, so this check skips a second run
			if (!this.editing) return;
			const newValue = parseFloat(this.text);
			this.updateValue(newValue);
			this.editing = false;
			const inputElement = this.$refs.input as HTMLElement;
			inputElement.blur();
		},
		onCancelTextChange() {
			this.updateValue(NaN);
			this.editing = false;
			const inputElement = this.$refs.input as HTMLElement;
			inputElement.blur();
		},
		onIncrement(direction: IncrementDirection) {
			if (Number.isNaN(this.value)) return;
			switch (this.incrementBehavior) {
				case "Add": {
					const directionAddend = direction === "Increase" ? this.incrementFactor : -this.incrementFactor;
					this.updateValue(this.value + directionAddend);
					break;
				}
				case "Multiply": {
					const directionMultiplier = direction === "Increase" ? this.incrementFactor : 1 / this.incrementFactor;
					this.updateValue(this.value * directionMultiplier);
					break;
				}
				case "Callback": {
					if (direction === "Increase" && this.incrementCallbackIncrease) this.incrementCallbackIncrease();
					if (direction === "Decrease" && this.incrementCallbackDecrease) this.incrementCallbackDecrease();
					break;
				}
				default:
					break;
			}
		},
		updateValue(newValue: number) {
			let sanitized = newValue;
			const invalid = Number.isNaN(newValue);
			if (invalid) sanitized = this.value;
			if (this.isInteger) sanitized = Math.round(sanitized);
			if (typeof this.min === "number" && !Number.isNaN(this.min)) sanitized = Math.max(sanitized, this.min);
			if (typeof this.max === "number" && !Number.isNaN(this.max)) sanitized = Math.min(sanitized, this.max);
			if (!invalid) this.$emit("update:value", sanitized);
			this.text = this.generateText(sanitized);
		},
		generateText(value: number): string {
			// Find the amount of digits on the left side of the decimal
			// 10.25 == 2
			// 1.23 == 1
			// 0.23 == 0 (Reason for the slightly more complicated code)
			const leftSideDigits = Math.max(Math.floor(value).toString().length, 0) * Math.sign(value);
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
			this.text = this.generateText(sanitized);
		},
	},
	mounted() {
		const inputElement = this.$refs.input as HTMLInputElement;
		inputElement.addEventListener("focus", this.onTextFocused);
		inputElement.addEventListener("blur", this.onTextChanged);
	},
	beforeUnmount() {
		const inputElement = this.$refs.input as HTMLInputElement;
		inputElement.removeEventListener("focus", this.onTextFocused);
		inputElement.removeEventListener("blur", this.onTextChanged);
	},
	components: { LayoutRow },
});
</script>
