<template>
	<LayoutRow class="window-button windows minimize" title="Minimize">
		<IconLabel :icon="'WindowButtonWinMinimize'" />
	</LayoutRow>
	<LayoutRow class="window-button windows maximize" title="Maximize" v-if="!maximized">
		<IconLabel :icon="'WindowButtonWinMaximize'" />
	</LayoutRow>
	<LayoutRow class="window-button windows restore-down" title="Restore Down" v-if="maximized">
		<IconLabel :icon="'WindowButtonWinRestoreDown'" />
	</LayoutRow>
	<LayoutRow class="window-button windows close" title="Close">
		<IconLabel :icon="'WindowButtonWinClose'" />
	</LayoutRow>
</template>

<style lang="scss">
.window-button.windows {
	flex: 0 0 auto;
	align-items: center;
	padding: 0 17px;

	svg {
		fill: var(--color-e-nearwhite);
	}

	&:hover {
		background: var(--color-6-lowergray);

		svg {
			fill: var(--color-f-white);
		}
	}

	&.close:hover {
		background: #e81123;
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	props: {
		maximized: { type: Boolean as PropType<boolean>, default: false },
	},
	components: {
		IconLabel,
		LayoutRow,
	},
});
</script>
