<!-- This is a base component, extended by others like NumberInput and TextInput. It should not be used directly. -->
<template>
	<LayoutRow class="field-input" :class="{ disabled }" :title="tooltip">
		<input
			v-if="!textarea"
			:class="{ 'has-label': label }"
			:id="`field-input-${id}`"
			ref="input"
			type="text"
			v-model="inputValue"
			:spellcheck="spellcheck"
			:disabled="disabled"
			@focus="() => $emit('textFocused')"
			@blur="() => $emit('textChanged')"
			@change="() => $emit('textChanged')"
			@keydown.enter="() => $emit('textChanged')"
			@keydown.esc="() => $emit('cancelTextChange')"
		/>
		<textarea
			v-else
			:class="{ 'has-label': label }"
			:id="`field-input-${id}`"
			class="scrollable-y"
			data-scrollable-y
			ref="input"
			v-model="inputValue"
			:spellcheck="spellcheck"
			:disabled="disabled"
			@focus="() => $emit('textFocused')"
			@blur="() => $emit('textChanged')"
			@change="() => $emit('textChanged')"
			@keydown.ctrl.enter="() => !macKeyboardLayout && $emit('textChanged')"
			@keydown.meta.enter="() => macKeyboardLayout && $emit('textChanged')"
			@keydown.esc="() => $emit('cancelTextChange')"
		></textarea>
		<label v-if="label" :for="`field-input-${id}`">{{ label }}</label>
		<slot></slot>
	</LayoutRow>
</template>

<style lang="scss">
.field-input {
	min-width: 80px;
	height: auto;
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
		white-space: nowrap;
	}

	&:not(.disabled) label {
		cursor: text;
	}

	input,
	textarea {
		flex: 1 1 100%;
		width: 0;
		min-width: 30px;
		height: 18px;
		line-height: 18px;
		margin: 0 8px;
		padding: 3px 0;
		outline: none; // Ok for input/textarea element
		border: none;
		background: none;
		color: var(--color-e-nearwhite);
		caret-color: var(--color-e-nearwhite);

		&::selection {
			background: var(--color-5-dullgray);
		}
	}

	input {
		text-align: center;

		&:not(:focus).has-label {
			text-align: right;
			margin-left: 0;
			margin-right: 8px;
		}

		&:focus {
			text-align: left;

			& + label {
				display: none;
			}
		}
	}

	textarea {
		min-height: calc(18px * 4);
		margin: 3px;
		padding: 0 5px;
		box-sizing: border-box;
		resize: vertical;
	}

	&.disabled {
		background: var(--color-2-mildblack);

		label,
		input,
		textarea {
			color: var(--color-8-uppergray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { platformIsMac } from "@/utility-functions/platform";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	emits: ["update:value", "textFocused", "textChanged", "cancelTextChange"],
	props: {
		value: { type: String as PropType<string>, required: true },
		label: { type: String as PropType<string>, required: false },
		spellcheck: { type: Boolean as PropType<boolean>, default: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		textarea: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },
	},
	data() {
		return {
			id: `${Math.random()}`.substring(2),
			macKeyboardLayout: platformIsMac(),
		};
	},
	methods: {
		// Select (highlight) all the text. For technical reasons, it is necessary to pass the current text.
		selectAllText(currentText: string) {
			const inputElement = this.$refs.input as HTMLInputElement | HTMLTextAreaElement | undefined;
			if (!inputElement) return;

			// Setting the value directly is required to make `inputElement.select()` work
			inputElement.value = currentText;

			inputElement.select();
		},
		unFocus() {
			(this.$refs.input as HTMLInputElement | HTMLTextAreaElement | undefined)?.blur();
		},
		getInputElementValue(): string | undefined {
			return (this.$refs.input as HTMLInputElement | HTMLTextAreaElement | undefined)?.value;
		},
		setInputElementValue(value: string) {
			const inputElement = this.$refs.input as HTMLInputElement | HTMLTextAreaElement | undefined;
			if (inputElement) inputElement.value = value;
		},
	},
	computed: {
		inputValue: {
			get() {
				return this.value;
			},
			set(value: string) {
				this.$emit("update:value", value);
			},
		},
	},
	components: { LayoutRow },
});
</script>
