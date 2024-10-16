+++
title = "Web-based vector graphics editor and design tool"
template = "section.html"

[extra]
css = ["index.css"]
js = ["image-interaction.js", "video-embed.js"]
+++

<!-- ▛ LOGO ▜ -->
<section id="logo">
<div class="block">
	<img src="https://static.graphite.rs/logos/graphite-logotype-color.svg" alt="Graphite Logo" />
</div>
</section>
<!-- ▙ LOGO ▟ -->

<!-- ▛ TAGLINE ▜ -->
<section id="tagline">
<div class="block">

<h1 class="balance-text">Your <span>procedural</span> toolbox for 2D content creation</h1>

<p class="balance-text">Graphite is a free, open source vector and raster graphics engine, available now in alpha. Get creative with a nondestructive editing workflow that combines layer-based compositing with node-based generative design.</p>

</div>
</section>
<!-- ▙ TAGLINE ▟ -->
<!--                -->
<!-- ▛ QUICK LINKS ▜ -->
<section id="quick-links">

<div class="call-to-action-buttons">
	<a href="https://github.com/GraphiteEditor/Graphite" class="button github-stars">
		<img src="https://static.graphite.rs/icons/github.svg" alt="GitHub" />
		<span class="arrow">Star</span>
		<div data-github-stars></div>
	</a>
	<a href="#newsletter" class="button arrow">Subscribe to newsletter</a>
</div>
<div class="social-media-buttons">
	<a href="https://discord.graphite.rs" target="_blank">
		<img src="https://static.graphite.rs/icons/discord__2.svg" alt="Discord" />
	</a>
	<a href="https://www.reddit.com/r/graphite/" target="_blank">
		<img src="https://static.graphite.rs/icons/reddit__2.svg" alt="Reddit" />
	</a>
	<a href="https://twitter.com/graphiteeditor" target="_blank">
		<img src="https://static.graphite.rs/icons/twitter.svg" alt="Twitter" />
	</a>
	<a href="https://www.youtube.com/@GraphiteEditor" target="_blank">
		<img src="https://static.graphite.rs/icons/youtube.svg" alt="YouTube" />
	</a>
</div>

</section>

<script>
(async () => {
	const element = document.querySelector("[data-github-stars]");
	try {
		const response = await fetch("https://api.github.com/repos/graphiteeditor/graphite?per_page=1");
		const json = await response.json();
		const stars = parseInt(json.stargazers_count);
		if (!stars) throw new Error();
		let quantity = stars.toLocaleString("en-US");
		if (quantity.length === 5) quantity = quantity.replace(",", "");
		element.innerText = quantity;
	} catch {
		element.remove();
	}
})();
</script>
<!-- ▙ QUICK LINKS ▟ -->

<!-- ▛ HEXAGONS ▜ -->
<div class="hexagons">
<div>

<svg viewBox="0 0 1400 1215.42" xmlns="http://www.w3.org/2000/svg">

<polygon points="1049.43,0.99 350.57,0.99 1.14,607.71 350.57,1214.44 1049.43,1214.44 1398.86,607.71" />
<polygon points="1016.39,57.57 383.61,57.57 67.22,607.71 383.61,1157.85 1016.39,1157.85 1332.78,607.71" />
<polygon points="964.49,149.01 435.51,149.01 171.02,607.71 435.51,1066.41 964.49,1066.41 1228.98,607.71" />
<polygon points="875.52,304.71 524.48,304.71 348.96,607.71 524.48,910.71 875.52,910.71 1051.04,607.71" />
<polygon points="768.12,490.96 631.88,490.96 563.78,607.71 631.88,724.47 768.12,724.47 836.22,607.71" />

</svg>

</div>
</div>
<!-- ▙ HEXAGONS ▟ -->

<!-- ▛ SCREENSHOTS ▜ -->
<section id="screenshots" class="carousel window-size-1" data-carousel data-carousel-jostle-hint>

<div class="carousel-slide" data-carousel-slide>
	<!-- Copy of last --><img src="https://static.graphite.rs/content/index/gui-mockup-nodes__7.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-demo-painted-dreams__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/magazine-page-layout.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-demo-node-graph-valley-of-spires__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-demo-fractal__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__7.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
	<!-- Copy of first --><img src="https://static.graphite.rs/content/index/gui-demo-painted-dreams__2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
</div>

<div class="carousel-slide torn left" data-carousel-slide-torn-left></div>
<div class="carousel-slide torn right" data-carousel-slide-torn-right></div>

<div class="screenshot-details">

<div class="carousel-controls">

<button class="direction prev" data-carousel-prev>

<svg width="40" height="40" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">

<path d="M20,0C8.95,0,0,8.95,0,20c0,11.05,8.95,20,20,20c11.05,0,20-8.95,20-20C40,8.95,31.05,0,20,0z M20,38c-9.93,0-18-8.07-18-18S10.07,2,20,2s18,8.07,18,18S29.93,38,20,38z" />
<polygon points="24.71,10.71 23.29,9.29 12.59,20 23.29,30.71 24.71,29.29 15.41,20" />

</svg>

</button>
<button class="dot active" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="dot" data-carousel-dot></button>
<button class="direction next" data-carousel-next>

<svg width="40" height="40" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">

<path d="M20,0C8.95,0,0,8.95,0,20c0,11.05,8.95,20,20,20c11.05,0,20-8.95,20-20C40,8.95,31.05,0,20,0z M20,38c-9.93,0-18-8.07-18-18S10.07,2,20,2s18,8.07,18,18S29.93,38,20,38z" />
<polygon points="16.71,9.29 15.29,10.71 24.59,20 15.29,29.29 16.71,30.71 27.41,20" />

</svg>

</button>

</div>
<div class="screenshot-description">

<p data-carousel-description class="active">
	<a href="https://editor.graphite.rs/#demo/painted-dreams"><em>Painted Dreams</em></a> — Made using nondestructive boolean operations and procedural dot patterns
</p>
<p data-carousel-description>
	Design for a magazine spread, a preview of the upcoming focus on desktop publishing
</p>
<p data-carousel-description>
	<a href="https://editor.graphite.rs/#demo/valley-of-spires"><em>Valley of Spires</em></a> — All layer stacks are represented, under the hood, by a node graph
</p>
<p data-carousel-description>
	Mandelbrot fractal filled with a noise pattern, procedurally generated and infinitely scalable
</p>
<p data-carousel-description>
	Coming soon: mockup for the actively in-development raster workflow with new nodes for photo editing
</p>

</div>

</div>
</section>

<!-- ▙ SCREENSHOTS ▟ -->
<!--                 -->
<!-- ▛ OVERVIEW ▜ -->
<section id="overview" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">Software overview</h1>

---

<!-- As a new entrant to the open source digital content creation landscape, Graphite has a unique formula for success: -->

Starting life as a vector editor, Graphite is evolving into a generalized, all-in-one graphics toolbox that's built more like a game engine than a conventional creative app. The editor's tools wrap its node graph core, providing user-friendly workflows for vector, raster, and beyond.

</div>
<div class="block workflows">

## One app to rule them all

Stop jumping between programs— upcoming tools will make Graphite a first-class content creation suite for many workflows, including:

<div class="feature-icons stacked no-background">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 12" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Graphic Design</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 13" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Image Editing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 17" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Motion Graphics</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 14" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Digital Painting</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 16" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>VFX Compositing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 15" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Desktop Publishing</span>
	</div>
</div>

</div>
<div class="diptych">

<div class="block">

## Current features

<div class="feature-icons">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 0" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Vector editing tools</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 10" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Procedural workflow for graphic design</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 8" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Node-based layers</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 3" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Forever free and open source</span>
	</div>
</div>

Presently, Graphite is a lightweight offline web app with features primarily oriented around procedural vector graphics editing.

</div>
<div class="block">

## Upcoming features

<div class="feature-icons">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 4" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>All-in-one creative tool for all things 2D</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 5" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Fully-featured raster manipulation</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 7" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Windows/Mac/Linux native apps + web</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 6" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span>Live collaborative editing</span>
	</div>
</div>

<a href="/features#roadmap" class="button arrow">Roadmap</a>

</div>

</div>
<div class="block">

## Desktop-first and web-ready

Graphite is designed principally as a desktop-grade professional application that is also accessible in-browser for fast, casual usage.

Where's the download? Desktop apps for Windows, Mac, and Linux should be available later in 2024. Until then, you can <a href="https://support.google.com/chrome/answer/9658361" target="_blank">install it as a PWA</a>.

Developing and maintaining a native app on four platforms is a big task. To not compromise on the optimal desktop experience—which takes longer to do the right way—priorities called for initially supporting just web, the one platform that stays up-to-date and reaches all devices.

Once it's ready to shine, Graphite's code architecture is structured to deliver native performance for your graphically intensive workloads on desktop platforms and very low overhead on the web thanks to WebAssembly and WebGPU, new high-performance browser technologies.

</div>

</div>
</section>
<!-- ▙ OVERVIEW ▟ -->
<!--                  -->
<!-- ▛ PROCEDURALISM ▜ -->
<section id="proceduralism" class="feature-box-outer">
<div class="feature-box-inner">

<div class="block">

<h1 class="feature-box-header">The power of proceduralism</h1>

---

Graphite is the first and only graphic design package built for procedural editing — where everything is nondestructive.

</div>

<div class="diptych red-dress">

<div class="block video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play>
		<source src="https://static.graphite.rs/content/index/procedural-demo-red-dress.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/index/procedural-demo-red-dress.mp4" type="video/mp4" />
	</video>
</div>

<div class="block description">

<h1 class="feature-box-header balance-text">Explore more creative avenues</h1>

Save hours on tedious alterations and make better creative choices. Graphite lets you iterate rapidly by adjusting node parameters instead of individual elements.

Scatter circles with just a couple nodes...  
Want them denser? Bigger? Those are sliders.  
Want a different placement area? Just tweak the path.

<a href="https://editor.graphite.rs/#demo/red-dress">Open this artwork</a> and give it a try yourself.

</div>

</div>
<div class="diptych leaves">

<div class="block description">

<h1 class="feature-box-header balance-text">Mix and morph parameters</h1>

Nondestructive editing means every decision is tied to a parameter you can adjust later on. Use Graphite to interpolate between any states just by dragging sliders.

Blend across color schemes. Morph shapes before they're scattered around the canvas. The possibilities are endless.

<a href="https://editor.graphite.rs/#demo/changing-seasons">Open this artwork</a> and give it a try yourself.

</div>

<div class="block video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play>
		<source src="https://static.graphite.rs/content/index/procedural-demo-leaves.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/index/procedural-demo-leaves.mp4" type="video/mp4" />
	</video>
</div>

</div>
<div class="block pipelines">

## Geared for generative pipelines

Graphite's representation of artwork as a node graph lets you customize, compose, reuse, share, and automate your own content workflows:

<div class="feature-icons four-wide">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 9" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Pixelation-free infinite zooming and panning of boundless content</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Modular node-based pipelines for generative AI <em>(soon)</em></span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-features__2.png" alt="" />
		<span class="balance-text">Asset pipelines for studio production environments <em>(soon)</em></span>
	</div>
</div>

</div>

</div>
</section>
<!-- ▙ PROCEDURALISM ▟ -->
<!--                 -->
<!-- ▛ DONATE ▜ -->
<section id="donate" class="block">

<div class="block">

## Support the mission

If you aren't paying for your free software, someone else is covering your share. Chip in so Graphite remains sustainable and independent.

<a href="https://github.com/sponsors/GraphiteEditor" class="button arrow">Donate</a>

</div>

</section>
<!-- ▙ DONATE ▟ -->
<!--                -->
<!-- ▛ NEWSLETTER ▜ -->
<section id="newsletter" class="feature-box-narrow">
<div id="newsletter-success"><!-- Used only as a URL hash fragment anchor --></div>

<div class="diptych">

<div class="block newsletter-signup">

<h1 class="feature-box-header">Stay in the loop</h1>

Subscribe to the newsletter for quarterly updates on major development progress. And follow along—or join the conversation—on social media.

<div class="newsletter-success">

## Thanks!

You'll receive your first newsletter email with the next major Graphite news.

</div>
<form action="https://graphite.rs/newsletter-signup" method="post">
	<div class="same-line">
		<div class="input-column name">
			<label for="newsletter-name">First + last name:</label>
			<input id="newsletter-name" name="name" type="text" required />
		</div>
		<div class="input-column phone">
			<label for="newsletter-phone">Phone:</label>
			<input id="newsletter-phone" name="phone" type="text" tabindex="-1" autocomplete="off" />
		</div>
		<div class="input-column email">
			<label for="newsletter-email">Email address:</label>
			<input id="newsletter-email" name="email" type="email" required />
		</div>
	</div>
	<div class="input-column submit">
		<input type="submit" value="Subscribe" class="button" />
	</div>
</form>

</div>
<div class="block social-media-links">

<a href="https://discord.graphite.rs" target="_blank">
	<img src="https://static.graphite.rs/icons/discord__2.svg" alt="Discord" />
	<span class="link not-uppercase arrow">Discord</span>
</a>
<a href="https://www.reddit.com/r/graphite/" target="_blank">
	<img src="https://static.graphite.rs/icons/reddit__2.svg" alt="Reddit" />
	<span class="link not-uppercase arrow">/r/Graphite</span>
</a>
<a href="https://twitter.com/graphiteeditor" target="_blank">
	<img src="https://static.graphite.rs/icons/twitter.svg" alt="Twitter" />
	<span class="link not-uppercase arrow">@GraphiteEditor</span>
</a>
<a href="https://www.youtube.com/@GraphiteEditor" target="_blank">
	<img src="https://static.graphite.rs/icons/youtube.svg" alt="YouTube" />
	<span class="link not-uppercase arrow">YouTube</span>
</a>

</div>

</div>
</section>
<!-- ▙ NEWSLETTER ▟ -->
<!--                -->
<!-- ▛ DIVE IN ▜ -->
<section id="dive-in" class="block">

<div class="block">

## Ready to dive in?

Get started with Graphite by following along to a hands-on quickstart tutorial.

<div class="block video-container">
<div>
<div class="video-embed aspect-16x9">
	<img data-video-embed="7gjUhl_3X10" src="https://static.graphite.rs/content/index/tutorial-1-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite Tutorial 1 - Hands-On Quickstart" />
</div>
</div>
</div>

<div class="buttons">
<a href="https://editor.graphite.rs" class="button arrow">Launch Graphite</a>
<a href="/learn" class="button arrow">Continue learning</a>
</div>

</div>

</section>
<!-- ▙ DIVE IN ▟ -->
<!--                 -->
<!-- ▛ RECENT NEWS ▜ -->
<section id="recent-news" class="feature-box-outer">
<div class="feature-box-inner">

<h1 class="feature-box-header">Recent news <span> / </span> <a href="/blog" class="link arrow">More in the blog</a></h1>

---

<div class="diptych">
<!-- replacements::blog_posts(count = 2) -->
</div>

</div>
</section>
<!-- ▙ RECENT NEWS ▟ -->
<!--                  -->
<!-- ▛ DEMO VIDEO ▜ -->
<!--
<section id="demo-video">
<div class="block">
Watch this timelapse showing the process of mixing traditional vector art (tracing a physical sketch and colorizing it, first two minutes) with using Imaginate to generate a background (last 45 seconds).
<div class="video-embed aspect-16x9">
	<img data-video-embed="JgJvAHQLnXA" src="https://static.graphite.rs/content/index/commander-basstronaut-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite - Vector Editing: &quot;Commander Basstronaut&quot; Artwork (25x Timelapse)" />
</div>
(Recorded in an older version of Graphite from early 2023.)
</div>
</section>
-->
<!-- ▙ DEMO VIDEO ▟ -->
<!--                 -->
<!-- ▛ IMAGINATE ▜ -->

<!-- TODO: Reenable when Imaginate is properly working again -->

<!--

<section id="imaginate">

<div class="block">

<h1><span class="alternating-text"><span>Co-create</span><span>Ideate</span><span>Illustrate</span><span>Generate</span><span>Iterate</span></span> with Imaginate</h1>

**Imaginate** is a node powered by <a href="https://en.wikipedia.org/wiki/Stable_Diffusion" target="_blank">Stable Diffusion</a> that makes AI-assisted art creation an easy, nondestructive process.
<!-- [Learn how](/learn/node-graph/imaginate) it works. --////////////////////>

</div>
<div class="diptych">

<div class="block">

<h2 class="balance-text">Add a touch of style</h2>

**Magically reimagine your vector drawings** in a fresh new style. Just place an Imaginate node between your layers and describe how it should end up looking.

<div class="image-comparison" data-image-comparison style="--comparison-percent: 50%">
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/light-bulb-before.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector illustration of a light bulb" />
	</div>
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/light-bulb-after.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Watercolor painting of a light bulb" />
	</div>
	<div class="slide-bar">
		<div class="arrows">
			<div></div>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
		</div>
	</div>
</div>

<blockquote class="balance-text require-polyfill"><strong>Watercolor painting</strong> of a light bulb gleaming with an exclamation mark inside</blockquote>

</div>
<div class="block">

## Work fast and sloppy

**Doodle a rough draft** without stressing over the details. Let Imaginate add the finishing touches to your artistic vision. Iterate with more passes until you're happy.

<div class="image-comparison" data-image-comparison style="--comparison-percent: 50%">
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/california-poppies-before.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Sloppy poppy: vector doodle of California poppy flowers wrapped around a circle" />
	</div>
	<div class="crop-container">
		<img src="https://static.graphite.rs/content/index/california-poppies-after.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Polished poppy: artistic, high-quality illustration of California poppy flowers wrapped around a circle" />
	</div>
	<div class="slide-bar">
		<div class="arrows">
			<div></div>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 22">
				<path d="M12.71 1.71 11.29.29.59 11l10.7 10.71 1.42-1.42L3.41 11Z" />
			</svg>
		</div>
	</div>
</div>

<blockquote class="balance-text require-polyfill"><strong>Botanical illustration</strong> of California poppies wrapped around a circle</blockquote>

</div>

</div>

</section>

-->

<!-- ▙ IMAGINATE ▟ -->
