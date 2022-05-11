<template>
	<LayoutRow :class="['icon-label', iconSize, iconStyle]">
		<component :is="icon" />
	</LayoutRow>
</template>

<style lang="scss">
.icon-label {
	flex: 0 0 auto;
	fill: var(--color-e-nearwhite);

	&.size-12 {
		width: 12px;
		height: 12px;
	}

	&.size-16 {
		width: 16px;
		height: 16px;
	}

	&.size-24 {
		width: 24px;
		height: 24px;
	}

	&.node-style {
		border-radius: 2px;
		background: var(--color-node-background);
		fill: var(--color-node-icon);
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { IconName, IconStyle, icons, iconComponents } from "@/utilities/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	props: {
		icon: { type: String as PropType<IconName>, required: true },
		gapAfter: { type: Boolean as PropType<boolean>, default: false },
		style: { type: String as PropType<IconStyle>, default: "" },
	},
	computed: {
		iconSize(): string {
			return `size-${icons[this.icon].size}`;
		},
		iconStyle(): string {
			if (!this.style) return "";
			return `${this.style}-style`;
		},
	},
	components: {
		LayoutRow,
		...iconComponents,
	},
});
</script>
