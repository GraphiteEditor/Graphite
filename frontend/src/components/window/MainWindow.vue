<template>
	<LayoutCol class="main-window">
		<LayoutRow :class="'title-bar-row'">
			<TitleBar :platform="platform" :maximized="maximized" />
		</LayoutRow>
		<LayoutRow :class="'workspace-row'">
			<Workspace />
		</LayoutRow>
		<LayoutRow :class="'status-bar-row'">
			<StatusBar />
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.main-window {
	min-height: 100%;
	// Creates a new stacking context for the app UI so that floating menus (which use `position: fixed` to leave their spawner element's stacking context) have an app-centric stacking context
	// Without this, floating menus would default to the web page's stacking context, which causes the floating menus to stay fixed when the page is scrolled and get offset from the app UI
	transform: translate(0, 0);
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
import TitleBar from "@/components/window/title-bar/TitleBar.vue";
import StatusBar from "@/components/window/status-bar/StatusBar.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import Workspace from "@/components/workspace/Workspace.vue";

export enum ApplicationPlatform {
	"Windows" = "Windows",
	"Mac" = "Mac",
	"Linux" = "Linux",
	"Web" = "Web",
}

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
			platform: ApplicationPlatform.Web,
			maximized: true,
		};
	},
});
</script>
