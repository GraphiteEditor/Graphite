<script lang="ts">
import { defineComponent, type PropType } from "vue";

import FieldInput from "@/components/widgets/inputs/FieldInput.vue";

export default defineComponent({
	emits: ["update:value", "commitText"],
	props: {
		value: { type: String as PropType<string>, required: true },
		label: { type: String as PropType<string>, required: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },
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
		// via the `change` event or when the <input> element is unfocused (with the `blur` event binding)
		onTextChanged() {
			// The `unFocus()` call in `onCancelTextChange()` causes itself to be run again, so this if statement skips a second run
			if (!this.editing) return;

			this.onCancelTextChange();

			// TODO: Find a less hacky way to do this
			const inputElement = this.$refs.fieldInput as typeof FieldInput | undefined;
			if (!inputElement) return;
			this.$emit("commitText", inputElement.getInputElementValue());

			// Required if value is not changed by the parent component upon update:value event
			inputElement.setInputElementValue(this.value);
		},
		onCancelTextChange() {
			this.editing = false;

			(this.$refs.fieldInput as typeof FieldInput | undefined)?.unFocus();
		},
	},
	components: { FieldInput },
});
</script>

<template>
	<FieldInput
		:textarea="true"
		class="text-area-input"
		:class="{ 'has-label': label }"
		:label="label"
		:spellcheck="true"
		:disabled="disabled"
		:tooltip="tooltip"
		v-model:value="inputValue"
		@textFocused="() => onTextFocused()"
		@textChanged="() => onTextChanged()"
		@cancelTextChange="() => onCancelTextChange()"
		ref="fieldInput"
	></FieldInput>
</template>

<style lang="scss"></style>
