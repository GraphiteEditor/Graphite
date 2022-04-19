<template>
	<!-- <LayoutRow class="text-area-input">
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
	</LayoutRow> -->
	<FieldInput
		:textarea="true"
		class="text-area-input"
		:class="{ 'has-label': label }"
		v-model:value="inputValue"
		:label="label"
		:spellcheck="true"
		:disabled="disabled"
		@textFocused="() => onTextFocused()"
		@textChanged="() => onTextChanged()"
		@cancelTextChange="() => onCancelTextChange()"
		ref="fieldInput"
	></FieldInput>
</template>

<style lang="scss"></style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import FieldInput from "@/components/widgets/inputs/FieldInput.vue";

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
			const inputElement = (this.$refs.fieldInput as typeof FieldInput).$refs.input as HTMLTextAreaElement;
			this.$emit("commitText", inputElement.value);

			// Required if value is not changed by the parent component upon update:value event
			inputElement.value = this.value;
		},
		onCancelTextChange() {
			this.editing = false;

			const inputElement = (this.$refs.fieldInput as typeof FieldInput).$refs.input as HTMLTextAreaElement;
			inputElement.blur();
		},
	},
	components: { FieldInput },
});
</script>
