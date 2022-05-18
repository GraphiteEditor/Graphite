<template>
	<div class="widget-layout">
		<component :is="layoutRowType(layoutRow)" :widgetData="layoutRow" :layoutTarget="layout.layout_target" v-for="(layoutRow, index) in layout.layout" :key="index" />
	</div>
</template>

<style lang="scss">
.widget-layout {
	height: 100%;
	flex: 0 0 auto;
	display: flex;
	flex-direction: column;
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { isWidgetColumn, isWidgetRow, isWidgetSection, LayoutRow, WidgetLayout } from "@/interop/messages";

import WidgetRow from "@/components/widgets/WidgetRow.vue";
import WidgetSection from "@/components/widgets/WidgetSection.vue";

export default defineComponent({
	props: {
		layout: { type: Object as PropType<WidgetLayout>, required: true },
	},
	methods: {
		layoutRowType(layoutRow: LayoutRow): unknown {
			if (isWidgetColumn(layoutRow)) return WidgetRow;
			if (isWidgetRow(layoutRow)) return WidgetRow;
			if (isWidgetSection(layoutRow)) return WidgetSection;

			throw new Error("Layout row type does not exist");
		},
	},
	data: () => ({
		isWidgetRow,
		isWidgetSection,
	}),
	components: {
		WidgetRow,
		WidgetSection,
	},
});
</script>
