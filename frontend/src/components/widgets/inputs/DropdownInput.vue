<template>
	<LayoutRow class="dropdown-input" data-dropdown-input>
		<LayoutRow
			class="dropdown-box"
			:class="{ disabled, open }"
			:style="{ minWidth: `${minWidth}px` }"
			@click="() => !disabled && (open = true)"
			@blur="(e: FocusEvent) => blur(e)"
			@keydown="(e: KeyboardEvent) => keydown(e)"
			ref="dropdownBox"
			tabindex="0"
			data-hover-menu-spawner
		>
			<IconLabel class="dropdown-icon" :icon="activeEntry.icon" v-if="activeEntry.icon" />
			<span>{{ activeEntry.label }}</span>
			<IconLabel class="dropdown-arrow" :icon="'DropdownArrow'" />
		</LayoutRow>
		<MenuList
			v-model:activeEntry="activeEntry"
			v-model:open="open"
			@naturalWidth="(newNaturalWidth: number) => (minWidth = newNaturalWidth)"
			:entries="entries"
			:drawIcon="drawIcon"
			:interactive="interactive"
			:direction="'Bottom'"
			:scrollableY="true"
			ref="menuList"
		/>
	</LayoutRow>
</template>

<style lang="scss">
.dropdown-input {
	position: relative;

	.dropdown-box {
		align-items: center;
		white-space: nowrap;
		background: var(--color-1-nearblack);
		height: 24px;
		border-radius: 2px;

		.dropdown-icon {
			margin: 4px;
			flex: 0 0 auto;
		}

		span {
			margin: 0;
			margin-left: 8px;
			flex: 1 1 100%;
		}

		.dropdown-icon + span {
			margin-left: 0;
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

			svg {
				fill: var(--color-f-white);
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

			svg {
				fill: var(--color-8-uppergray);
			}
		}
	}

	.menu-list .floating-menu-container .floating-menu-content {
		max-height: 400px;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType, toRaw } from "vue";

import { MenuListEntry } from "@/wasm-communication/messages";

import MenuList from "@/components/floating-menus/MenuList.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

const DASH_ENTRY = { label: "-" };

export default defineComponent({
	emits: ["update:selectedIndex"],
	props: {
		entries: { type: Array as PropType<MenuListEntry[][]>, required: true },
		selectedIndex: { type: Number as PropType<number>, required: false }, // When not provided, a dash is displayed
		drawIcon: { type: Boolean as PropType<boolean>, default: false },
		interactive: { type: Boolean as PropType<boolean>, default: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			activeEntry: this.makeActiveEntry(this.selectedIndex),
			activeEntrySkipWatcher: false,
			open: false,
			minWidth: 0,
		};
	},
	watch: {
		// Called only when `selectedIndex` is changed from outside this component (with v-model)
		selectedIndex() {
			this.activeEntrySkipWatcher = true;
			this.activeEntry = this.makeActiveEntry();
		},
		// Called when `activeEntry` is changed by the `v-model` on this component's MenuList component, or by the `selectedIndex()` watcher above (but we want to skip that case)
		activeEntry(newActiveEntry: MenuListEntry) {
			if (this.activeEntrySkipWatcher) {
				this.activeEntrySkipWatcher = false;
				return;
			}

			// `toRaw()` pulls it out of the Vue proxy
			if (toRaw(newActiveEntry) === DASH_ENTRY) return;

			this.$emit("update:selectedIndex", this.entries.flat().indexOf(newActiveEntry));
		},
	},
	methods: {
		makeActiveEntry(): MenuListEntry {
			const entries = this.entries.flat();

			if (this.selectedIndex !== undefined && this.selectedIndex >= 0 && this.selectedIndex < entries.length) {
				return entries[this.selectedIndex];
			}
			return DASH_ENTRY;
		},
		keydown(e: KeyboardEvent) {
			(this.$refs.menuList as typeof MenuList).keydown(e, false);
		},
		blur(e: FocusEvent) {
			if ((e.target as HTMLElement).closest("[data-dropdown-input]") !== this.$el) this.open = false;
		},
	},
	components: {
		IconLabel,
		MenuList,
		LayoutRow,
	},
});
</script>
