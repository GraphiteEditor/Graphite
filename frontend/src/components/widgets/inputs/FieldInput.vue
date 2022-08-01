<!-- This is a base component, extended by others like NumberInput and TextInput. It should not be used directly. -->
<template>
	<LayoutRow class="field-input" :class="{ disabled }">
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
		outline: none;
		border: none;
		background: none;
		color: var(--color-e-nearwhite);
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
import { defineComponent, PropType } from "vue";

import { operatingSystemIsMac } from "@/utility-functions/platform";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	emits: ["update:value", "textFocused", "textChanged", "cancelTextChange"],
	props: {
		value: { type: String as PropType<string>, required: true },
		label: { type: String as PropType<string>, required: false },
		spellcheck: { type: Boolean as PropType<boolean>, default: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		textarea: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			id: `${Math.random()}`.substring(2),
			macKeyboardLayout: operatingSystemIsMac(),
		};
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
