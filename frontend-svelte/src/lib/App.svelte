<script lang="ts" context="module">
export type ApplicationPlatform = "Windows" | "Mac" | "Linux" | "Web"
</script>

<script lang="ts">
import LayoutCol from "$lib/c/layout/LayoutCol.svelte"
import Workspace from "$lib/c/window/workspace/Workspace.svelte"
import TitleBar from "$lib/c/window/title-bar/TitleBar.svelte"
import StatusBar from "$lib/c/window/status-bar/StatusBar.svelte"
</script>

<LayoutCol>
  <TitleBar platform="Web" maximized="{true}" />

  <Workspace />

  <StatusBar />
</LayoutCol>
