<script lang="ts">
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import { dataPanelContextLayout, dataPanelLayout, dataPanelPathLayout } from "/src/stores/portfolio";
</script>

<LayoutCol class="data-panel">
	<LayoutRow class="top-bar">
		<LayoutRow class="path" scrollableX={true}>
			<WidgetLayout layout={dataPanelPathLayout} layoutTarget="DataPanelPath" />
		</LayoutRow>
		<LayoutRow class="context" scrollableX={true}>
			<WidgetLayout layout={dataPanelContextLayout} layoutTarget="DataPanelContext" />
		</LayoutRow>
	</LayoutRow>
	<LayoutCol class="body" scrollableX={true} scrollableY={true}>
		<WidgetLayout layout={dataPanelLayout} layoutTarget="DataPanel" />
	</LayoutCol>
</LayoutCol>

<style lang="scss">
	.data-panel {
		flex-grow: 1;
		padding-bottom: 4px;

		.top-bar {
			flex: 0 0 auto;
			padding: 0 4px;

			.path,
			.context {
				flex: 1 1 50%;
				min-width: 0;
			}

			// An auto margin, unlike end alignment, keeps the start of overflowing content reachable by scrolling
			.context > :first-child {
				margin-left: auto;
			}
		}

		// Within the scrolling area, so the table's gutter can reach its edges without overflowing
		.body {
			padding: 0 4px;
		}

		table {
			margin: -4px;
			width: calc(100% + 2 * 4px);

			// Values stay on one line, cut off past a cap, and the panel scrolls sideways when the columns outgrow it
			td > .widget-span > :is(.text-label, .text-button) {
				max-width: 400px;
			}

			td > .widget-span > .text-label {
				overflow: hidden;
				text-overflow: ellipsis;
			}

			&:not(:first-child) {
				margin-top: -4px;
			}

			tr:first-child:has(td:first-child label:empty) ~ tr td:first-child {
				width: 0;
			}
		}

		.widget-span:has(.text-area-input) {
			flex: 1 1 100%;

			.text-area-input {
				margin: 0;
				padding: 4px 0;

				textarea {
					height: 100%;
					margin-top: 0;
					margin-bottom: 0;
					resize: none;
				}
			}
		}
	}
</style>
