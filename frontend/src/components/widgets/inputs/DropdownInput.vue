<template>
	<div class="dropdown-input">
		<div class="dropdown-box" :class="{ disabled }" :style="{ minWidth: `${minWidth}px`, disabled: 'disabled' }" @click="() => clickDropdownBox()" data-hover-menu-spawner>
			<IconLabel :class="'dropdown-icon'" :icon="activeEntry.icon" v-if="activeEntry.icon" />
			<span>{{ activeEntry.label }}</span>
			<IconLabel :class="'dropdown-arrow'" :icon="'DropdownArrow'" />
		</div>
		<MenuList
			v-model:activeEntry="activeEntry"
			@update:activeEntry="(newActiveEntry) => activeEntryChanged(newActiveEntry)"
			@widthChanged="(newWidth) => onWidthChanged(newWidth)"
			:menuEntries="menuEntries"
			:direction="'Bottom'"
			:drawIcon="drawIcon"
			:scrollable="true"
			ref="menuList"
		/>
	</div>
</template>

<style lang="scss">
.dropdown-input {
	position: relative;

	.dropdown-box {
		display: flex;
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
			display: inline-block;
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
import { defineComponent, PropType } from "vue";

import MenuList, { MenuListEntry, SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";

export default defineComponent({
	props: {
		menuEntries: { type: Array as PropType<SectionsOfMenuListEntries>, required: true },
		selectedIndex: { type: Number as PropType<number>, required: true },
		drawIcon: { type: Boolean as PropType<boolean>, default: false },
		disabled: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			activeEntry: this.menuEntries.flat()[this.selectedIndex],
			minWidth: 0,
		};
	},
	watch: {
		// Called only when `selectedIndex` is changed from outside this component (with v-model)
		selectedIndex(newSelectedIndex: number) {
			const entries = this.menuEntries.flat();

			if (!Number.isNaN(newSelectedIndex) && newSelectedIndex >= 0 && newSelectedIndex < entries.length) {
				this.activeEntry = entries[newSelectedIndex];
			} else {
				this.activeEntry = { label: "-" };
			}
		},
	},
	methods: {
		// Called only when `activeEntry` is changed from the child MenuList component via user input
		activeEntryChanged(newActiveEntry: MenuListEntry) {
			this.$emit("update:selectedIndex", this.menuEntries.flat().indexOf(newActiveEntry));
		},
		clickDropdownBox() {
			if (!this.disabled) (this.$refs.menuList as typeof MenuList).setOpen();
		},
		onWidthChanged(newWidth: number) {
			this.minWidth = newWidth;
		},
	},
	components: {
		IconLabel,
		MenuList,
	},
});
</script>
