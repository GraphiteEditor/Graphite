<template>
	<LayoutRow class="row">
		<textarea
			:class="{ 'has-label': label }"
			:id="`field-input-${id}`"
			ref="input"
			type="text"
			v-model="inputValue"
			:spellcheck="true"
			:disabled="disabled"
			@focus="() => onTextFocused()"
			@blur="() => onTextChanged()"
			@change="() => onTextChanged()"
			@keydown.esc="() => onCancelTextChange()"
		/>
		<label v-if="label" :for="`field-input-${id}`">{{ label }}</label>
	</LayoutRow>
</template>

<style lang="scss">
.row {
	min-width: 80px;
	min-height: 24px;
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

	textarea {
		text-align: left;
		flex: 1 1 100%;
		width: 0;
		min-width: 30px;
		line-height: 18px;
		height: 18px;
		margin: 0 8px;
		padding: 3px 0;
		outline: none;
		border: none;
		background: none;
		color: var(--color-e-nearwhite);

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

	&.disabled {
		background: var(--color-2-mildblack);

		label,
		input {
			color: var(--color-8-uppergray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	emits: ["update:value", "commitText"],
	props: {
		value: { type: String as PropType<string>, required: true },
		label: { type: String as PropType<string>, required: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			editing: false,
			id: `${Math.random()}`.substring(2),
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
	methods: {
		onTextFocused() {
			this.editing = true;
		},
		// Called only when `value` is changed from the <textarea> element via user input and committed, either
		// via the `change` event or when the <input> element is defocused (with the `blur` event binding)
		onTextChanged() {
			// The `inputElement.blur()` call in `onCancelTextChange()` causes itself to be run again, so this if statement skips a second run
			if (!this.editing) return;

			this.onCancelTextChange();

			// TODO: Find a less hacky way to do this
			const inputElement = this.$refs.input as HTMLTextAreaElement;
			this.$emit("commitText", inputElement.value);

			// Required if value is not changed by the parent component upon update:value event
			inputElement.value = this.value;
		},
		onCancelTextChange() {
			this.editing = false;

			const inputElement = this.$refs.input as HTMLTextAreaElement;
			inputElement.blur();
		},
	},
	components: { LayoutRow },
});
</script>
