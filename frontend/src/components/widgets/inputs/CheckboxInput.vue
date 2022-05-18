<template>
	<LayoutRow class="checkbox-input" :class="{ 'outline-style': outlineStyle }">
		<input type="checkbox" :id="`checkbox-input-${id}`" :checked="checked" @change="(e) => $emit('update:checked', (e.target as HTMLInputElement).checked)" />
		<label :for="`checkbox-input-${id}`">
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

	&.outline-style label {
		.checkbox-box {
			border: 1px solid var(--color-e-nearwhite);
			padding: 1px;
			background: none;

			svg {
				display: none;
			}
		}

		&:hover .checkbox-box {
			border: 1px solid var(--color-f-white);
		}
	}

	&.outline-style input:checked + label {
		.checkbox-box {
			background: none;

			svg {
				display: block;
				fill: var(--color-e-nearwhite);
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IconName } from "@/utilities/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	emits: ["update:checked"],
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
	props: {
		checked: { type: Boolean as PropType<boolean>, default: false },
		icon: { type: String as PropType<IconName>, default: "Checkmark" },
		outlineStyle: { type: Boolean as PropType<boolean>, default: false },
	},
	components: {
		IconLabel,
		LayoutRow,
	},
});
</script>
