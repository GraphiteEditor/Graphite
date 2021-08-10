<template>
	<div class="number-input" :class="{ disabled }">
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
		<button v-if="!Number.isNaN(value)" class="arrow left" @click="onIncrement(IncrementDirection.Decrease)"></button>
		<button v-if="!Number.isNaN(value)" class="arrow right" @click="onIncrement(IncrementDirection.Increase)"></button>
	</div>
</template>

<style lang="scss">
.number-input {
	width: 80px;
	height: 24px;
	position: relative;
	border-radius: 2px;
	background: var(--color-1-nearblack);
	overflow: hidden;
	display: flex;
	flex-direction: row-reverse;

	label {
		flex: 0 0 auto;
		cursor: text;
		line-height: 18px;
		margin-left: 8px;
		padding: 3px 0;
	}

	input {
		flex: 1 1 100%;
		width: 100%;
		height: 18px;
		line-height: 18px;
		margin: 0 8px;
		padding: 3px 0;
		outline: none;
		border: none;
		background: none;
		color: var(--color-e-nearwhite);
		font-size: inherit;
		font-family: inherit;
		text-align: center;

		&:not(:focus).has-label {
			text-align: right;
			padding-left: 4px;
			margin-left: 0;
			margin-right: 8px;
		}

		&::selection {
			background: var(--color-accent);
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

export enum IncrementBehavior {
	Add = "Add",
	Multiply = "Multiply",
	Callback = "Callback",
	None = "None",
}

export enum IncrementDirection {
	Decrease = "Decrease",
	Increase = "Increase",
}

export default defineComponent({
	components: {},
	props: {
		value: { type: Number, required: true },
		min: { type: Number, required: false },
		max: { type: Number, required: false },
		incrementBehavior: { type: String as PropType<IncrementBehavior>, default: IncrementBehavior.Add },
		incrementFactor: { type: Number, default: 1 },
		incrementCallbackIncrease: { type: Function, required: false },
		incrementCallbackDecrease: { type: Function, required: false },
		isInteger: { type: Boolean, default: false },
		unit: { type: String, default: "" },
		unitIsHiddenWhenEditing: { type: Boolean, default: true },
		displayDecimalPlaces: { type: Number, default: 3 },
		label: { type: String, required: false },
		disabled: { type: Boolean, default: false },
	},
	data() {
		return {
			text: `${this.value}${this.unit}`,
			editing: false,
			IncrementDirection,
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
				case IncrementBehavior.Add: {
					const directionAddend = direction === IncrementDirection.Increase ? this.incrementFactor : -this.incrementFactor;
					this.updateValue(this.value + directionAddend);
					break;
				}
				case IncrementBehavior.Multiply: {
					const directionMultiplier = direction === IncrementDirection.Increase ? this.incrementFactor : 1 / this.incrementFactor;
					this.updateValue(this.value * directionMultiplier);
					break;
				}
				case IncrementBehavior.Callback: {
					if (direction === IncrementDirection.Increase && this.incrementCallbackIncrease) this.incrementCallbackIncrease();
					if (direction === IncrementDirection.Decrease && this.incrementCallbackDecrease) this.incrementCallbackDecrease();
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

			const roundingPower = 10 ** this.displayDecimalPlaces;
			const displayValue = Math.round(sanitized * roundingPower) / roundingPower;
			this.text = `${displayValue}${this.unit}`;
		},
	},
	watch: {
		// Called only when `value` is changed from outside this component (with v-model)
		value(newValue: number) {
			if (Number.isNaN(newValue)) {
				this.text = "-";
				return;
			}

			let sanitized = newValue;
			if (typeof this.min === "number") sanitized = Math.max(sanitized, this.min);
			if (typeof this.max === "number") sanitized = Math.min(sanitized, this.max);

			const roundingPower = 10 ** this.displayDecimalPlaces;
			const displayValue = Math.round(sanitized * roundingPower) / roundingPower;
			this.text = `${displayValue}${this.unit}`;
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
});
</script>
