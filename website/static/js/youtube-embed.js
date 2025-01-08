window.addEventListener("DOMContentLoaded", () => {
	document.querySelectorAll("[data-video-embed]").forEach((placeholder) => {
		placeholder.addEventListener("click", () => {
			const videoId = placeholder.attributes["data-video-embed"].value;
			placeholder.outerHTML = `<iframe width="1280" height="720" src="https://www.youtube.com/embed/${videoId}?autoplay=1" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" allowfullscreen></iframe>`;
		});
	});
});
