<script lang="ts">
	import { untrack } from "svelte";
	import WidgetSpan from "/src/components/widgets/WidgetSpan.svelte";
	import type { LayoutTarget, WidgetTable } from "/wrapper/pkg/graphite_wasm_wrapper";

	// The height of a row and the spacing between rows, as styled below
	const ROW_HEIGHT = 32;
	const ROW_SPACING = 4;
	const ROW_PITCH = ROW_HEIGHT + ROW_SPACING;
	// Rows this far past either end of the view are requested too, so scrolling doesn't uncover them before they arrive
	const REQUESTED_BEYOND_VIEW_PIXELS = 720;
	// Rows are requested anew once the view comes this close to an end of those sent, rather than on every scroll
	const REQUEST_AHEAD_PIXELS = 240;

	type Props = {
		widgetData: WidgetTable;
		layoutTarget: LayoutTarget;
		// Asks for the rows of a virtually scrolled table from `start` up to but not including `end`
		requestRows?: (start: number, end: number) => void;
	};
	let { widgetData, layoutTarget, requestRows }: Props = $props();

	let table: HTMLTableElement | undefined = $state();

	// The rows last requested, so scrolling further before they arrive doesn't ask for them again
	let requested: { start: number; end: number } | undefined = undefined;

	// The widest content seen in each column, kept as the column's minimum so it doesn't narrow when its widest rows scroll away
	let columnContentWidths: number[] = $state([]);
	// The row and column counts the widths were seen with, since a table with different counts starts over
	let columnContentWidthsTable = "";

	const columns = $derived(widgetData.tableWidgets.length > 0 ? widgetData.tableWidgets[0].length : 0);

	const virtuallyScrolled = $derived(Boolean(widgetData.scrollWindow));
	// The rows sent after the heading row, from `start` up to but not including `end`, out of the table's `total`
	const rowsSent = $derived.by(() => {
		const scrollWindow = widgetData.scrollWindow;
		if (!scrollWindow) return undefined;

		const start = scrollWindow.rowsBefore;
		const end = start + widgetData.tableWidgets.length - 1;
		return { start, end, total: end + scrollWindow.rowsAfter };
	});

	// Follows the scrolling and resizing of the area the table scrolls within
	$effect(() => {
		const scroller = virtuallyScrolled ? table?.closest("[data-scrollable-y]") : undefined;
		if (!(scroller instanceof HTMLElement)) return;

		const resizeObserver = new ResizeObserver(requestRowsInView);
		resizeObserver.observe(scroller);
		scroller.addEventListener("scroll", requestRowsInView, { passive: true });

		return () => {
			resizeObserver.disconnect();
			scroller.removeEventListener("scroll", requestRowsInView);
		};
	});

	// The rows sent may not be those in view, such as after the table's length changes
	$effect(() => {
		if (!rowsSent) return;

		untrack(widenColumnsToFitRowsSent);
		requested = undefined;
		requestRowsInView();
	});

	// Raises each column's minimum width to fit the content of the rows sent. O(cells sent)
	function widenColumnsToFitRowsSent() {
		if (!table || !rowsSent) return;

		const tableCounts = `${rowsSent.total} rows, ${columns} columns`;
		const widths = tableCounts === columnContentWidthsTable ? [...columnContentWidths] : [];
		columnContentWidthsTable = tableCounts;

		// Each cell's content is measured as the span of its widgets, rather than the cell, which stretches to fill the panel
		const range = document.createRange();
		Array.from(table.rows)
			.slice(1)
			.filter((row) => !row.classList.contains("unrendered-rows"))
			.forEach((row) => {
				Array.from(row.cells).forEach((cell, column) => {
					if (!cell.firstElementChild) return;

					range.selectNodeContents(cell.firstElementChild);
					widths[column] = Math.max(widths[column] || 0, Math.ceil(range.getBoundingClientRect().width));
				});
			});

		const widened = widths.length !== columnContentWidths.length || widths.some((width, column) => width !== columnContentWidths[column]);
		if (widened) columnContentWidths = widths;
	}

	// Asks for the rows in and near view once the view nears an end of the rows sent. O(1)
	function requestRowsInView() {
		const scroller = table?.closest("[data-scrollable-y]");
		const headingRow = table?.rows[0];
		if (!rowsSent || !requestRows || !table || !headingRow || !(scroller instanceof HTMLElement)) return;
		const { total } = rowsSent;

		// The view's top and bottom, measured down from the top of the first row after the heading row, found from the table's top since the heading row sticks
		const firstRowTop = table.getBoundingClientRect().top + ROW_SPACING + headingRow.offsetHeight + ROW_SPACING;
		const viewTop = scroller.getBoundingClientRect().top - firstRowTop;
		const viewBottom = viewTop + scroller.clientHeight;

		const rowsCovering = (top: number, bottom: number) => {
			const clamp = (row: number) => Math.min(Math.max(row, 0), total);
			return { start: clamp(Math.floor(top / ROW_PITCH)), end: clamp(Math.ceil(bottom / ROW_PITCH)) };
		};

		const needed = rowsCovering(viewTop - REQUEST_AHEAD_PIXELS, viewBottom + REQUEST_AHEAD_PIXELS);
		if (needed.start >= rowsSent.start && needed.end <= rowsSent.end) return;

		const { start, end } = rowsCovering(viewTop - REQUESTED_BEYOND_VIEW_PIXELS, viewBottom + REQUESTED_BEYOND_VIEW_PIXELS);
		if (requested?.start === start && requested?.end === end) return;

		requested = { start, end };
		requestRows(start, end);
	}
</script>

<table class:unstyled={widgetData.unstyled} class:virtually-scrolled={virtuallyScrolled} bind:this={table}>
	<tbody>
		{#each widgetData.tableWidgets as row, rowIndex}
			<tr>
				{#each row as cell, column}
					<td colspan={row.length < columns ? columns - row.length + 1 : undefined}>
						<WidgetSpan direction="row" widgets={cell} {layoutTarget} narrow={true} />
						{#if rowIndex === 0 && virtuallyScrolled && columnContentWidths[column]}
							<div class="column-content-width" style:width={`${columnContentWidths[column]}px`}></div>
						{/if}
					</td>
				{/each}
			</tr>
			<!-- Stand-ins as tall as the unsent rows before and after those sent, which follow the heading row -->
			{#if rowIndex === 0 && widgetData.scrollWindow && widgetData.scrollWindow.rowsBefore > 0}
				<tr class="unrendered-rows" style:height={`${widgetData.scrollWindow.rowsBefore * ROW_PITCH - ROW_SPACING}px`}>
					{#each { length: columns }, column (column)}<td></td>{/each}
				</tr>
			{/if}
		{/each}
		{#if widgetData.scrollWindow && widgetData.scrollWindow.rowsAfter > 0}
			<tr class="unrendered-rows" style:height={`${widgetData.scrollWindow.rowsAfter * ROW_PITCH - ROW_SPACING}px`}>
				{#each { length: columns }, column (column)}<td></td>{/each}
			</tr>
		{/if}
	</tbody>
</table>

<style lang="scss">
	// Row elements show different rows as those sent shift, so the browser mustn't scroll to keep one in place
	table.virtually-scrolled {
		overflow-anchor: none;
	}

	table:not(.unstyled) {
		// Beneath the spacing color's shadow, the cells' color is what the browser shows for parts not yet drawn during fast scrolls
		background: var(--color-2-mildblack);
		box-shadow: inset 0 0 0 100000px var(--color-3-darkgray);
		border: none;
		border-spacing: 4px;
		border-radius: 2px;

		td {
			background: var(--color-2-mildblack);
			vertical-align: top;
			border: none;
			border-radius: 2px;
			padding: 4px 8px;
		}

		// The heading row stays in view as the others scroll beneath it, with the spacing around its cells filled in to hide them
		tr:first-child td {
			background-image: var(--inheritance-dots-background-4-dimgray);
			position: sticky;
			top: 0;
			z-index: 1;
			box-shadow: 0 0 0 4px var(--color-3-darkgray);
		}

		// Each column's tall stand-in is striped into what look like empty cells, one per unsent row
		tr.unrendered-rows td {
			padding: 0;
			border-radius: 0;
			background: repeating-linear-gradient(var(--color-2-mildblack) 0 32px, transparent 32px 36px);
		}

		.column-content-width {
			height: 0;
		}
	}
</style>
