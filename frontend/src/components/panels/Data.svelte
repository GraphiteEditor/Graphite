<script lang="ts">
	import { getContext } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { PortfolioStore } from "/src/stores/portfolio";

	const portfolio = getContext<PortfolioStore>("portfolio");
</script>

<LayoutCol class="data-panel">
	<LayoutCol class="body" scrollableY={true}>
		<WidgetLayout layout={$portfolio.dataPanelLayout} layoutTarget="DataPanel" />
	</LayoutCol>
</LayoutCol>

<style lang="scss">
	.data-panel {
		flex-grow: 1;
		padding: 4px;
		padding-top: 0;

		table {
			margin: -4px;
			width: calc(100% + 2 * 4px);

			.text-label {
				white-space: wrap;
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
