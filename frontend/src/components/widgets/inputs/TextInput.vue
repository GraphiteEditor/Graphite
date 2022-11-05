<template>
	<FieldInput
		class="text-input"
		:class="{ centered }"
		v-model:value="text"
		:label="label"
		:spellcheck="true"
		:disabled="disabled"
		:tooltip="tooltip"
		:style="minWidth > 0 ? `min-width: ${minWidth}px` : ''"
		:sharpRightCorners="sharpRightCorners"
		@textFocused="() => onTextFocused()"
		@textChanged="() => onTextChanged()"
		@cancelTextChange="() => onCancelTextChange()"
		ref="fieldInput"
	></FieldInput>
</template>

<style lang="scss">
.text-input {
	input {
		text-align: left;
	}

	&.centered {
		input {
			text-align: center;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import FieldInput from "@/components/widgets/inputs/FieldInput.vue";

export default defineComponent({
	emits: ["update:value", "commitText"],
	props: {
		// Label
		label: { type: String as PropType<string>, required: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },

		// Disabled
		disabled: { type: Boolean as PropType<boolean>, default: false },

		// Value
		value: { type: String as PropType<string>, required: true },

		// Styling
		centered: { type: Boolean as PropType<boolean>, default: false },
		minWidth: { type: Number as PropType<number>, default: 0 },
		sharpRightCorners: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			editing: false,
		};
	},
	computed: {
		text: {
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

			(this.$refs.fieldInput as typeof FieldInput | undefined)?.selectAllText(this.text);
		},
		// Called only when `value` is changed from the <input> element via user input and committed, either with the
		// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding)
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
