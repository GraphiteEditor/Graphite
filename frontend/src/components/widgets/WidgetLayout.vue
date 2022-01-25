<template>
	<div class="widget-layout">
		<template v-for="(layoutRow, index) in layout.layout" :key="index">
			<WidgetRow v-if="isWidgetRow(layoutRow)" :widgetRow="layoutRow" :layoutTarget="layout.layout_target" />
			<WidgetSection v-if="isWidgetSection(layoutRow)" :WidgetSection="layoutRow" :layoutTarget="layout.layout_target" />
		</template>
	</div>
</template>

<style lang="scss">
.widget-layout {
	height: 100%;
	flex: 0 0 auto;
	display: flex;
	flex-direction: column;
	align-items: center;
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { isWidgetRow, isWidgetSection, WidgetLayout } from "@/dispatcher/js-messages";

import WidgetRow from "@/components/widgets/WidgetRow.vue";
import WidgetSection from "@/components/widgets/WidgetSection.vue";

export default defineComponent({
	props: {
		layout: { type: Object as PropType<WidgetLayout>, required: true },
	},
	data: () => {
		return {
			isWidgetRow,
			isWidgetSection,
		};
	},
	components: {
		WidgetRow,
		WidgetSection,
	},
});
</script>

