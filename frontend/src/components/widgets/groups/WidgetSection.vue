<!-- TODO: Implement collapsable sections with properties system -->
<template>
	<LayoutCol class="widget-section">
		<button class="header" :class="{ expanded }" @click.stop="() => (expanded = !expanded)" tabindex="0">
			<div class="expand-arrow"></div>
			<TextLabel :bold="true">{{ widgetData.name }}</TextLabel>
		</button>
		<LayoutCol class="body" v-if="expanded">
			<component :is="layoutGroupType(layoutRow)" :widgetData="layoutRow" :layoutTarget="layoutTarget" v-for="(layoutRow, index) in widgetData.layout" :key="index"></component>
		</LayoutCol>
	</LayoutCol>
</template>

<style lang="scss">
.widget-section {
	flex: 0 0 auto;

	.header {
		text-align: left;
		align-items: center;
		display: flex;
		flex: 0 0 24px;
		padding: 0 8px;
		margin-bottom: 4px;
		border: 0;
		border-radius: 4px;
		background: var(--color-5-dullgray);

		.expand-arrow {
			width: 8px;
			height: 8px;
			margin: 0;
			padding: 0;
			position: relative;
			flex: 0 0 auto;
			display: flex;
			align-items: center;
			justify-content: center;

			&::after {
				content: "";
				position: absolute;
				width: 8px;
				height: 8px;
				background: var(--icon-expand-collapse-arrow);
			}
		}

		&.expanded {
			border-radius: 4px 4px 0 0;
			margin-bottom: 0;

			.expand-arrow::after {
				transform: rotate(90deg);
			}
		}

		.text-label {
			height: 18px;
			margin-left: 8px;
			display: inline-block;
		}

		&:hover {
			background: var(--color-6-lowergray);

			.expand-arrow::after {
				background: var(--icon-expand-collapse-arrow-hover);
			}

			.text-label {
				color: var(--color-f-white);
			}

			+ .body {
				border: 1px solid var(--color-6-lowergray);
			}
		}
	}

	.body {
		padding: 0 7px;
		padding-top: 1px;
		margin-top: -1px;
		margin-bottom: 4px;
		border: 1px solid var(--color-5-dullgray);
		border-radius: 0 0 4px 4px;

		.widget-row {
			&:first-child {
				margin-top: -1px;
			}

			&:last-child {
				margin-bottom: -1px;
			}

			> .text-label:first-of-type {
				flex: 0 0 30%;
				text-align: right;
			}

			> .text-button {
				flex-grow: 1;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { isWidgetRow, isWidgetSection, type LayoutGroup, type WidgetSection as WidgetSectionFromJsMessages } from "@/wasm-communication/messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";
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
		updateLayout(widgetId: bigint, value: unknown) {
			this.editor.instance.updateLayout(this.layoutTarget, widgetId, value);
		},
		layoutGroupType(layoutGroup: LayoutGroup): unknown {
			if (isWidgetRow(layoutGroup)) return WidgetRow;
			if (isWidgetSection(layoutGroup)) return WidgetSection;

			throw new Error("Layout row type does not exist");
		},
	},
	components: {
		LayoutCol,
		LayoutRow,
		TextLabel,
		WidgetRow,
	},
});
export default WidgetSection;
</script>
