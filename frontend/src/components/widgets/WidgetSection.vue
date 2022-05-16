<!-- TODO: Implement collapsable sections with properties system -->
<template>
	<LayoutCol class="widget-section">
		<button class="header" @click.stop="() => (expanded = !expanded)">
			<div class="expand-arrow" :class="{ expanded }"></div>
			<Separator :type="'Related'" />
			<TextLabel :bold="true">{{ widgetData.name }}</TextLabel>
		</button>
		<LayoutCol class="body" v-if="expanded">
			<component :is="layoutRowType(layoutRow)" :widgetData="layoutRow" :layoutTarget="layoutTarget" v-for="(layoutRow, index) in widgetData.layout" :key="index"></component>
		</LayoutCol>
	</LayoutCol>
</template>

<style lang="scss">
.widget-section {
	flex: 0 0 auto;

	.header {
		display: flex;
		flex-direction: row;
		flex-grow: 1;
		min-width: 0;
		min-height: 0;
		border: 0;
		padding: 0;
		text-align: left;

		flex: 0 0 24px;
		background: var(--color-4-dimgray);
		align-items: center;
		padding: 0 8px;
		margin: 0 -4px;

		.expand-arrow {
			width: 6px;
			height: 100%;
			padding: 0;
			position: relative;
			flex: 0 0 auto;
			display: flex;
			align-items: center;
			justify-content: center;

			&::after {
				content: "";
				position: absolute;
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 0 3px 6px;
				border-color: transparent transparent transparent var(--color-e-nearwhite);
			}

			&.expanded::after {
				border-width: 6px 3px 0 3px;
				border-color: var(--color-e-nearwhite) transparent transparent transparent;
			}
		}

		.text-label {
			height: 18px;
			display: inline-block;
		}
	}

	.body {
		margin: 0 4px;

		.text-label {
			flex: 0 0 30%;
			text-align: right;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { isWidgetRow, isWidgetSection, LayoutRow as LayoutSystemRow, WidgetSection as WidgetSectionFromJsMessages } from "@/dispatcher/js-messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";
import Separator from "@/components/widgets/separators/Separator.vue";
import WidgetRow from "@/components/widgets/WidgetRow.vue";

const WidgetSection = defineComponent({
	name: "WidgetSection",
	inject: ["editor"],
	props: {
		widgetData: { type: Object as PropType<WidgetSectionFromJsMessages>, required: true },
		layoutTarget: { required: true },
	},
	data: () => ({
		isWidgetRow,
		isWidgetSection,
		expanded: true,
	}),
	methods: {
		updateLayout(widgetId: BigInt, value: unknown) {
			this.editor.instance.update_layout(this.layoutTarget, widgetId, value);
		},
		layoutRowType(layoutRow: LayoutSystemRow): unknown {
			if (isWidgetRow(layoutRow)) return WidgetRow;
			if (isWidgetSection(layoutRow)) return WidgetSection;

			throw new Error("Layout row type does not exist");
		},
	},
	components: {
		LayoutCol,
		LayoutRow,
		TextLabel,
		Separator,
		WidgetRow,
	},
});
export default WidgetSection;
</script>
