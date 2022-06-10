<template>
	<LayoutRow class="font-input">
		<LayoutRow class="dropdown-box" :class="{ disabled }" :style="{ minWidth: `${minWidth}px` }" tabindex="0" @click="toggleOpen" @keydown="keydown" data-hover-menu-spawner>
			<span>{{ activeEntry?.value || "" }}</span>
			<IconLabel class="dropdown-arrow" :icon="'DropdownArrow'" />
		</LayoutRow>
		<MenuList
			ref="menulist"
			v-model:activeEntry="activeEntry"
			v-model:open="open"
			:entries="[entries]"
			:minWidth="isStyle ? 0 : minWidth"
			:virtualScrollingEntryHeight="isStyle ? 0 : 20"
			:scrollableY="true"
			@naturalWidth="(newNaturalWidth: number) => (isStyle && (minWidth = newNaturalWidth))"
		></MenuList>
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

		.spacer {
			flex: 0 0 auto;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, nextTick, PropType } from "vue";

import FloatingMenu from "@/components/floating-menus/FloatingMenu.vue";
import MenuList, { MenuListEntry } from "@/components/floating-menus/MenuList.vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

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
			entries: [] as MenuListEntry[],
			activeEntry: undefined as MenuListEntry | undefined,
			highlighted: undefined as MenuListEntry | undefined,
			entriesStart: 0,
			minWidth: this.isStyle ? 0 : 300,
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
			return ((this.$refs.menulist as typeof MenuList).$refs.scroller as typeof LayoutCol)?.$el as HTMLElement;
		},
		async setOpen() {
			this.open = true;
			// Scroll to the active entry (the scroller div does not yet exist so we must wait for vue to render)
			await nextTick();
			if (this.activeEntry) {
				const index = this.entries.indexOf(this.activeEntry);
				this.scroller()?.scrollTo(0, Math.max(0, index * 20 - 190));
			}
		},
		toggleOpen() {
			if (this.disabled) return;
			this.open = !this.open;
			if (this.open) this.setOpen();
		},
		keydown(e: KeyboardEvent) {
			(this.$refs.menulist as typeof MenuList).keydown(e, false);
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
		async getEntries(): Promise<MenuListEntry[]> {
			const x = this.isStyle ? this.fonts.getFontStyles(this.fontFamily) : this.fonts.fontNames();
			return (await x).map((entry: { name: string; url: URL | undefined }) => ({
				label: entry.name,
				value: entry.name,
				font: entry.url,
				action: () => this.selectFont(entry.name),
			}));
		},
		getActiveEntry(entries: MenuListEntry[]): MenuListEntry {
			const selectedChoice = this.isStyle ? this.fontStyle : this.fontFamily;

			return entries.find((entry) => entry.value === selectedChoice) as MenuListEntry;
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
	components: {
		LayoutRow,
		IconLabel,
		MenuList,
	},
});
</script>
