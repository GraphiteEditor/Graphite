<template>
	<LayoutRow class="font-input">
		<LayoutRow class="dropdown-box" :class="{ disabled }" :style="{ minWidth: `${minWidth}px` }" tabindex="0" @click="toggleOpen" @keydown="keydown" data-hover-menu-spawner>
			<span>{{ activeEntry?.name || "" }}</span>
			<IconLabel class="dropdown-arrow" :icon="'DropdownArrow'" />
		</LayoutRow>
		<FloatingMenu class="menu-list" v-model:open="open" :direction="'Bottom'" :type="'Dropdown'" ref="floatingMenu" :windowEdgeMargin="0" data-hover-menu-keep-open>
			<LayoutCol :scrollableY="true" @scroll="onScroll" :style="{ width: `${minWidth}px` }" ref="scroller">
				<LayoutRow class="spacer" :style="{ height: `${startIndex * 20}px` }"></LayoutRow>
				<LayoutRow
					v-for="(entry, entryIndex) in entries.slice(startIndex, endIndex)"
					:key="entryIndex + startIndex"
					class="row"
					:class="{ active: entry === highlighted }"
					@click="selectFont(entry.name)"
				>
					<link v-if="!isStyle" rel="stylesheet" :href="entry.url?.toString()" />

					<span class="entry-label" :style="{ fontFamily: `${isStyle ? 'inherit' : entry.name}` }">{{ entry.name }}</span>
				</LayoutRow>
				<LayoutRow class="spacer" :style="{ height: `${totalHeight - endIndex * 20}px` }"></LayoutRow>
			</LayoutCol>
		</FloatingMenu>
	</LayoutRow>
</template>

<style lang="scss">
.font-input {
	position: relative;

	.dropdown-box {
		align-items: center;
		white-space: nowrap;
		background: var(--color-1-nearblack);
		height: 24px;
		border-radius: 2px;

		span {
			margin: 0;
			margin-left: 8px;
			flex: 1 1 100%;
		}

		.dropdown-arrow {
			margin: 6px 2px;
			flex: 0 0 auto;
		}

		&:hover,
		&.open {
			background: var(--color-6-lowergray);

			span {
				color: var(--color-f-white);
			}
		}

		&.open {
			border-radius: 2px 2px 0 0;
		}

		&.disabled {
			background: var(--color-2-mildblack);

			span {
				color: var(--color-8-uppergray);
			}
		}
	}

	.menu-list .floating-menu-container .floating-menu-content {
		max-height: 400px;

		padding: 4px 0;

		.row {
			height: 20px;
			align-items: center;
			white-space: nowrap;
			position: relative;
			flex: 0 0 auto;
		}

		.spacer {
			flex: 0 0 auto;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, nextTick, PropType } from "vue";

import FloatingMenu from "@/components/floating-menus/FloatingMenu.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

interface FontEntry {
	name: string;
	url: URL | undefined;
}

export default defineComponent({
	inject: ["fonts"],
	emits: ["update:fontFamily", "update:fontStyle", "changeFont"],
	props: {
		fontFamily: { type: String as PropType<string>, required: true },
		fontStyle: { type: String as PropType<string>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		isStyle: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			open: false,
			entries: [] as FontEntry[],
			activeEntry: undefined as FontEntry | undefined,
			highlighted: undefined as FontEntry | undefined,
			entriesStart: 0,
			minWidth: 300,
		};
	},
	async mounted() {
		this.entries = await this.getEntries();
		this.activeEntry = this.getActiveEntry(this.entries);
		this.highlighted = this.activeEntry;
	},
	methods: {
		floatingMenu() {
			return this.$refs.floatingMenu as typeof FloatingMenu;
		},
		scroller() {
			return (this.$refs.scroller as typeof LayoutCol)?.$el as HTMLElement;
		},
		setOpen() {
			this.open = true;
			// Reset the highlighted entry to the active one
			this.setHighlighted(this.activeEntry);
			// Scroll to the active entry (the scroller div does not yet exist so we must wait for vue to render)
			nextTick((): void => {
				if (this.activeEntry) {
					const index = this.entries.indexOf(this.activeEntry);
					this.scroller()?.scrollTo(0, Math.max(0, index * 20 - 190));
				}
			});
		},
		toggleOpen() {
			if (this.disabled) return;
			this.open = !this.open;
			if (this.open) this.setOpen();
		},
		async selectFont(newName: string): Promise<void> {
			let fontFamily;
			let fontStyle;

			if (this.isStyle) {
				this.$emit("update:fontStyle", newName);

				fontFamily = this.fontFamily;
				fontStyle = newName;
			} else {
				this.$emit("update:fontFamily", newName);

				fontFamily = newName;
				fontStyle = "Normal (400)";
			}

			const fontFileUrl = await this.fonts.getFontFileUrl(fontFamily, fontStyle);
			this.$emit("changeFont", { fontFamily, fontStyle, fontFileUrl });
		},
		onWidthChanged(newWidth: number) {
			this.minWidth = newWidth;
		},
		async getEntries(): Promise<FontEntry[]> {
			return this.isStyle ? this.fonts.getFontStyles(this.fontFamily) : this.fonts.fontNames();
		},
		getActiveEntry(entries: FontEntry[]): FontEntry {
			const selectedChoice = this.isStyle ? this.fontStyle : this.fontFamily;

			return entries.find((entry) => entry.name === selectedChoice) as FontEntry;
		},
		/// Handles keyboard navigation for the menu. Returns if the entire menu stack should be dismissed
		keydown(e: KeyboardEvent) {
			if (this.disabled) return;

			const menuOpen = this.open;

			if (!menuOpen && (e.key === " " || e.key === "Enter")) {
				// Allow opening menu with space or enter
				this.setOpen();
			} else if (menuOpen && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
				// Navigate to the next and previous entries with arrow keys

				let newIndex = e.key === "ArrowUp" ? this.entries.length - 1 : 0;
				if (this.highlighted) {
					const index = this.highlighted ? this.entries.indexOf(this.highlighted) : 0;
					newIndex = (index + (e.key === "ArrowUp" ? -1 : 1) + this.entries.length) % this.entries.length;
				}

				const newEntry = this.entries[newIndex];
				this.setHighlighted(newEntry);
			} else if (menuOpen && e.key === "Escape") {
				// Close menu with escape key
				this.open = false;

				// Reset active to before open
				this.setHighlighted(this.activeEntry);
			} else if (menuOpen && this.highlighted && e.key === "Enter") {
				// Handle clicking on an option if enter is pressed
				this.selectFont(this.highlighted.name);
				e.preventDefault();
			}
		},
		setHighlighted(newHighlight: FontEntry | undefined) {
			this.highlighted = newHighlight;
		},
		onScroll(e: Event) {
			this.entriesStart = (e.target as HTMLElement)?.scrollTop || 0;
		},
	},
	watch: {
		async fontFamily() {
			this.entries = await this.getEntries();
			this.activeEntry = this.getActiveEntry(this.entries);
			this.highlighted = this.activeEntry;
		},
		async fontStyle() {
			this.entries = await this.getEntries();
			this.activeEntry = this.getActiveEntry(this.entries);
			this.highlighted = this.activeEntry;
		},
	},
	computed: {
		totalHeight() {
			return this.entries.length * 20;
		},
		startIndex() {
			return Math.floor(this.entriesStart / 20);
		},
		endIndex() {
			return Math.min(this.entries.length, this.startIndex + 1 + 400 / 20);
		},
	},
	components: {
		LayoutRow,
		LayoutCol,
		IconLabel,
		FloatingMenu,
	},
});
</script>
