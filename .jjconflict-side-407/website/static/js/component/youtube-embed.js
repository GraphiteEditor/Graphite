window.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll("[data-youtube-embed]").forEach((placeholder) => {
		if (!(placeholder instanceof HTMLElement)) return;

		const loadIframe = () => {
			const videoId = placeholder.getAttribute("data-youtube-embed") || "";
			const timestamp = placeholder.getAttribute("data-youtube-timestamp") || "";

			const iframe = document.createElement("iframe");
			iframe.width = "1280";
			iframe.height = "720";
			iframe.src = `https://www.youtube.com/embed/${videoId}?autoplay=1${timestamp ? `&start=${timestamp}` : ""}`;
			iframe.allow = "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture";
			iframe.allowFullscreen = true;

			const embed = placeholder.closest("[data-youtube-embed]");
			embed?.replaceChildren(iframe);
		};

		placeholder.addEventListener("click", loadIframe);
		placeholder.addEventListener("keydown", (event) => {
			if (event.key === "Enter" || event.key === " ") {
				event.preventDefault();
				loadIframe();
			}
		});
	});
});
