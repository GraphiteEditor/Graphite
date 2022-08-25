<template>
	<LayoutRow class="checkbox-input">
		<input type="checkbox" :id="`checkbox-input-${id}`" :checked="checked" @change="(e) => $emit('update:checked', (e.target as HTMLInputElement).checked)" />
		<label :for="`checkbox-input-${id}`" tabindex="0" @keydown.enter="(e) => ((e.target as HTMLElement).previousSibling as HTMLInputElement).click()" :title="tooltip">
			<LayoutRow class="checkbox-box">
				<IconLabel :icon="icon" />
			</LayoutRow>
		</label>
	</LayoutRow>
</template>

<style lang="scss">
.checkbox-input {
	flex: 0 0 auto;
	align-items: center;

	input {
		display: none;
	}

	label {
		display: flex;
		height: 16px;
		// Provides rounded corners for the :focus outline
		border-radius: 2px;

		.checkbox-box {
			flex: 0 0 auto;
			background: var(--color-e-nearwhite);
			padding: 2px;
			border-radius: 2px;

			.icon-label {
				fill: var(--color-2-mildblack);
			}
		}

		&:hover .checkbox-box {
			background: var(--color-f-white);
		}
	}

	input:checked + label {
		.checkbox-box {
			background: var(--color-accent);

			.icon-label {
				fill: var(--color-f-white);
			}
		}

		&:hover .checkbox-box {
			background: var(--color-accent-hover);
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
		icon: { type: String as PropType<IconName>, default: "Checkmark" },
		tooltip: { type: String as PropType<string | undefined>, required: false },
	},
	data() {
		return {
			id: `${Math.random()}`.substring(2),
		};
	},
	methods: {
		isChecked() {
			return this.checked;
		},
	},
	components: {
		IconLabel,
		LayoutRow,
	},
});
</script>
