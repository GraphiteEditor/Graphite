<!-- TODO: Implement Collapsable sections with properties system -->
<template>
	<div class="widget-section">
		<template v-for="(layoutRow, index) in WidgetSection.layout" :key="index">
			<WidgetRow v-if="isWidgetRow(layoutRow)" :widgetRow="layoutRow" :layoutTarget="layoutTarget" />
			<WidgetSection v-if="isWidgetSection(layoutRow)" :WidgetSection="layoutRow" :layoutTarget="layoutTarget" />
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

import { isWidgetRow, isWidgetSection, WidgetSection } from "@/dispatcher/js-messages";

import WidgetRow from "@/components/widgets/WidgetRow.vue";

export default defineComponent({
	name: "WidgetSection",
	inject: ["editor"],
	props: {
		WidgetSection: { type: Object as PropType<WidgetSection>, required: true },
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
	},
	components: {
		WidgetRow,
	},
});
</script>

