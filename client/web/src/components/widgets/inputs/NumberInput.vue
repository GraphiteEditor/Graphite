<template>
	<div class="number-input" :class="{ disabled }">
		<input type="text" spellcheck="false" v-model="text" @change="onTextChanged()" @keydown.esc="onCancelTextChange" ref="input" :disabled="disabled" />
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

	input {
		width: calc(100% - 8px);
		line-height: 18px;
		margin: 3px 4px;
		outline: none;
		border: none;
		background: none;
		padding: 0;
		color: var(--color-e-nearwhite);
		font-size: inherit;
		text-align: center;
		font-family: inherit;

		&::selection {
			background: var(--color-accent);
		}

		&:focus {
			text-align: left;
			width: calc(100% - 16px);
			margin-left: 8px;
			margin-right: 8px;

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
		outline: none;
		border: none;
		background: none;
		padding: 9px 0;

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
import { defineComponent } from "vue";

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
		step: { type: Number, default: 1 },
		stepIsMultiplier: { type: Boolean, default: false },
		isInteger: { type: Boolean, default: false },
		unit: { type: String, default: "" },
		unitIsHiddenWhenEditing: { type: Boolean, default: true },
		displayDecimalPlaces: { type: Number, default: 3 },
		disabled: { type: Boolean, default: false },
	},
	data() {
		return {
			text: `${this.value}${this.unit}`,
			editing: false,
			IncrementDirection,
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

			if (this.stepIsMultiplier) {
				const directionMultiplier = direction === IncrementDirection.Increase ? this.step : 1 / this.step;
				this.updateValue(this.value * directionMultiplier);
			} else {
				const directionAddend = direction === IncrementDirection.Increase ? this.step : -this.step;
				this.updateValue(this.value + directionAddend);
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
