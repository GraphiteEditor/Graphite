window.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll("[data-youtube-embed]").forEach((placeholder) => {
		if (!(placeholder instanceof HTMLElement)) return;
		placeholder.addEventListener("click", () => {
			const videoId = placeholder.getAttribute("data-youtube-embed") || "";
			const timestamp = placeholder.getAttribute("data-youtube-timestamp") || "";
			placeholder.outerHTML = `
				<iframe \\
				width="1280" \\
				height="720" \\
				src="https://www.youtube.com/embed/${videoId}?${timestamp ? `start=${timestamp}&` : ""}autoplay=1" \\
				frameborder="0" \\
				allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" \\
				allowfullscreen\\
				>\\
				</iframe>\\
				`
				.split("\n")
				.map((line) => line.trim())
				.join("")
				.replaceAll(`\\`, "");
		});
	});
});
