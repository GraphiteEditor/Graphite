<!-- TODO: Implement collapsable sections with properties system -->
<template>
	<div class="widget-section">
		<template v-for="(layoutRow, index) in widgetData.layout" :key="index">
			<component :is="layoutRowType(layoutRow)" :widgetData="layoutRow" :layoutTarget="layoutTarget"></component>
		</template>
	</div>
</template>

<style lang="scss">
.widget-section {
	height: 100%;
	flex: 0 0 auto;
	display: flex;
	align-items: center;
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { isWidgetRow, isWidgetSection, LayoutRow, WidgetSection as WidgetSectionFromJsMessages } from "@/dispatcher/js-messages";

import WidgetRow from "@/components/widgets/WidgetRow.vue";

const WidgetSection = defineComponent({
	name: "WidgetSection",
	inject: ["editor"],
	props: {
		widgetData: { type: Object as PropType<WidgetSectionFromJsMessages>, required: true },
		layoutTarget: { required: true },
	},
	data: () => {
		return {
			isWidgetRow,
			isWidgetSection,
		};
	},
	methods: {
		updateLayout(widgetId: BigInt, value: unknown) {
			this.editor.instance.update_layout(this.layoutTarget, widgetId, value);
		},
		layoutRowType(layoutRow: LayoutRow): unknown {
			if (isWidgetRow(layoutRow)) return WidgetRow;
			if (isWidgetSection(layoutRow)) return WidgetSection;

			throw new Error("Layout row type does not exist");
		},
	},
	components: {
		WidgetRow,
	},
});
export default WidgetSection;
</script>

