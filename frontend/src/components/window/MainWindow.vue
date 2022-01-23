<template>
	<LayoutCol class="main-window">
		<LayoutRow class="title-bar-row">
			<TitleBar :platform="platform" :maximized="maximized" />
		</LayoutRow>
		<LayoutRow class="workspace-row">
			<Workspace />
		</LayoutRow>
		<LayoutRow class="status-bar-row">
			<StatusBar />
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.main-window {
	height: 100%;
	overflow: auto;
	touch-action: none;
}

.title-bar-row {
	height: 28px;
	flex: 0 0 auto;
}

.workspace-row {
	position: relative;
	flex: 1 1 100%;
}

.status-bar-row {
	flex: 0 0 auto;
	// Prevents the creation of a scrollbar due to the child's negative margin
	overflow: hidden;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import StatusBar from "@/components/window/status-bar/StatusBar.vue";
import TitleBar from "@/components/window/title-bar/TitleBar.vue";
import Workspace from "@/components/workspace/Workspace.vue";

export type ApplicationPlatform = "Windows" | "Mac" | "Linux" | "Web";

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		TitleBar,
		Workspace,
		StatusBar,
	},
	data() {
		return {
			platform: "Web" as ApplicationPlatform,
			maximized: true,
		};
	},
});
</script>
