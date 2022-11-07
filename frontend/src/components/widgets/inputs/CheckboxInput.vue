<template>
	<LayoutRow class="checkbox-input">
		<input
			type="checkbox"
			:id="`checkbox-input-${id}`"
			:checked="checked"
			@change="(e) => $emit('update:checked', (e.target as HTMLInputElement).checked)"
			:disabled="disabled"
			:tabindex="disabled ? -1 : 0"
		/>
		<label :class="{ disabled, checked }" :for="`checkbox-input-${id}`" @keydown.enter="(e) => toggleCheckboxFromLabel(e)" :title="tooltip">
			<LayoutRow class="checkbox-box">
				<IconLabel :icon="displayIcon" />
			</LayoutRow>
		</label>
	</LayoutRow>
</template>

<style lang="scss">
.checkbox-input {
	flex: 0 0 auto;
	align-items: center;

	input {
		// We can't use `display: none` because it must be visible to work as a tabbale input that accepts a space bar actuation
		width: 0;
		height: 0;
		margin: 0;
		opacity: 0;
	}

	// Unchecked
	label {
		display: flex;
		height: 16px;
		// Provides rounded corners for the :focus outline
		border-radius: 2px;

		.checkbox-box {
			flex: 0 0 auto;
			background: var(--color-5-dullgray);
			padding: 2px;
			border-radius: 2px;

			.icon-label {
				fill: var(--color-8-uppergray);
			}
		}

		// Hovered
		&:hover .checkbox-box {
			background: var(--color-6-lowergray);
		}

		// Disabled
		&.disabled .checkbox-box {
			background: var(--color-4-dimgray);
		}
	}

	// Checked
	input:checked + label {
		.checkbox-box {
			background: var(--color-e-nearwhite);

			.icon-label {
				fill: var(--color-2-mildblack);
			}
		}

		// Hovered
		&:hover .checkbox-box {
			background: var(--color-f-white);
		}

		// Hovered
		&.disabled .checkbox-box {
			background: var(--color-8-uppergray);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type IconName } from "@/utility-functions/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	emits: ["update:checked"],
	props: {
		checked: { type: Boolean as PropType<boolean>, default: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		icon: { type: String as PropType<IconName>, default: "Checkmark" },
		tooltip: { type: String as PropType<string | undefined>, required: false },
	},
	data() {
		return {
			id: `${Math.random()}`.substring(2),
		};
	},
	computed: {
		displayIcon(): IconName {
			if (!this.checked && this.icon === "Checkmark") return "Empty12px";

			return this.icon;
		},
	},
	methods: {
		isChecked() {
			return this.checked;
		},
		toggleCheckboxFromLabel(e: KeyboardEvent) {
			const target = (e.target || undefined) as HTMLLabelElement | undefined;
			const previousSibling = (target?.previousSibling || undefined) as HTMLInputElement | undefined;
			previousSibling?.click();
		},
	},
	components: {
		IconLabel,
		LayoutRow,
	},
});
</script>
