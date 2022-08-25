<template>
	<LayoutRow :class="['icon-label', iconSizeClass, iconStyleClass]">
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
import { defineComponent, type PropType } from "vue";

import { type IconName, type IconStyle, ICONS, ICON_COMPONENTS } from "@/utility-functions/icons";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	props: {
		icon: { type: String as PropType<IconName>, required: true },
		iconStyle: { type: String as PropType<IconStyle | undefined>, required: false },
	},
	computed: {
		iconSizeClass(): string {
			return `size-${ICONS[this.icon].size}`;
		},
		iconStyleClass(): string {
			if (!this.iconStyle || this.iconStyle === "Normal") return "";
			return `${this.iconStyle.toLowerCase()}-style`;
		},
	},
	components: {
		LayoutRow,
		...ICON_COMPONENTS,
	},
});
</script>
