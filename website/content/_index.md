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

<h1 class="balance-text">Redefining state&#8209;of&#8209;the&#8209;art graphics editing</h1>

<p class="balance-text"><strong>Graphite</strong> is an in-development vector and raster graphics editor that's free and open source. It is powered by a node graph compositor that fuses layers with nodes and brings a unique procedural approach to your 2D design workflow.</p>

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

<!-- ▛ SCREENSHOTS ▜ -->
<section id="screenshots" class="carousel window-size-1" data-carousel data-carousel-jostle-hint>
	<div class="carousel-slide" data-carousel-slide>
		<!-- Copy of last --><img src="https://static.graphite.rs/content/index/gui-mockup-viewport__5.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-demo-red-dress.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #1" style="transform: translateX(-100%)" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-demo-valley-of-spires__4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #2" style="transform: translateX(-100%)" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-demo-procedural-string-lights.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #3" style="transform: translateX(-100%)" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__5.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #4" style="transform: translateX(-100%)" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-viewport__5.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #5" style="transform: translateX(-100%)" data-carousel-image />
		<!-- Copy of first --><img src="https://static.graphite.rs/content/index/gui-demo-red-dress.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" style="transform: translateX(-100%)" data-carousel-image />
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
			<p class="active" data-carousel-description>
				<a href="https://editor.graphite.rs/#demo/red-dress"><em>Red Dress</em></a> — Illustration made with the help of procedurally generating hundreds of circles in the node graph.
			</p>
			<p data-carousel-description>
				<a href="https://editor.graphite.rs/#demo/valley-of-spires"><em>Valley of Spires</em></a> — Vector art made with the Pen and Gradient tools without needing to touch the node graph.
			</p>
			<p data-carousel-description>
				<a href="https://editor.graphite.rs/#demo/procedural-string-lights"><em>Procedural String Lights</em></a> — Drawing of a tree adorned by reusable, auto-placed light bulbs along the wire path made using this node graph.
			</p>
			<p data-carousel-description>
				Design mockup for the work-in-progress raster editing pipeline. Some of these raster-specific nodes are not implemented yet, but will be soon!
			</p>
			<p data-carousel-description>
				Design mockup for the work-in-progress raster editing workflow. Some of the features shown here are not implemented yet, but will be soon!
			</p>
		</div>
	</div>
</section>

<!-- ▙ SCREENSHOTS ▟ -->
<!--                      -->
<!-- ▛ TODAY AND TOMORROW ▜ -->
<section id="today-and-tomorrow">
<div class="diptych">

<div class="block">

# Graphite today <span class="status-flag">public alpha</span>

<div class="feature-icons">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 0" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Vector art editing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 8" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Node-based layers</span>
	</div>
	<div class="feature-icon">
		<!-- TODO: Reenable when Imaginate is properly working again -->
		<!-- <img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" /> -->
		<!-- <span>AI-assisted art creation</span> -->
		<img class="atlas" style="--atlas-index: 10" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Procedural graphic design workflow</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 3" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Forever free and open source</span>
	</div>
</div>

<!-- Presently, Graphite is a lightweight vector graphics editor that runs offline in your browser (no sign up or download required). -->
Presently, Graphite is a lightweight offline web app with features primarily oriented around procedural vector graphics editing.

</div>
<div class="block">

# Graphite tomorrow

<div class="feature-icons">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 4" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>All-in-one creative tool for all things 2D</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 5" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Clean, familiar, designer-centric UI</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 7" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Multiplatform app for desktop + iPad</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 6" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Live collaborative editing</span>
	</div>
</div>

<a href="/features#roadmap" class="button arrow">Roadmap</a>

</div>

</div>
</section>
<!-- ▙ TODAY AND TOMORROW ▟ -->
<!--                     -->
<!-- ▛ DISCIPLINES ▜ -->
<section id="disciplines">
<div class="block">

# One app to rule them all

**Stop jumping between programs. Planned features will make Graphite a first-class design tool for all these disciplines.** <small>*(Listed by priority.)*</small>

<div class="feature-icons stacked no-background">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 12" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Graphic Design</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 13" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Image Editing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 17" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Motion Graphics</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 14" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Digital Painting</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 16" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>VFX Compositing</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 15" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Desktop Publishing</span>
	</div>
</div>

</div>
</section>
<!-- ▙ DISCIPLINES ▟ -->
<!--                  -->
<!-- ▛ NEWSLETTER ▜ -->
<section id="newsletter" class="feature-box-outer">
<div id="newsletter-success"><!-- Used only as a URL hash fragment anchor --></div>
<div class="feature-box-inner">

<h1 class="feature-box-header">Stay in the loop</h1>

---

<div class="diptych">

<div class="block newsletter-signup">

**Subscribe to the newsletter** for quarterly updates on major development progress. And follow along—or join the conversation—on social media.

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
</div>
</section>
<!-- ▙ NEWSLETTER ▟ -->
<!--                   -->
<!-- ▛ JUMP RIGHT IN ▜ -->
<!-- <section id="jump-right-in">
<div class="block"> -->

<!-- # Jump right in -->

<!-- **Get started with Graphite by following along to a hands-on quickstart tutorial.** -->

<!-- <div class="video-embed aspect-16x9">
	<img data-video-embed="7gjUhl_3X10" src="https://static.graphite.rs/content/index/tutorial-1-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite Tutorial 1 - Hands-On Quickstart" />
</div> -->

<!-- </div>
</section> -->
<!-- ▙ JUMP RIGHT IN ▟ -->
<!--                    -->
<!-- ▛ PROCEDURALISM ▜ -->
<section id="proceduralism">
<div class="block">

# Powerful proceduralism

**Graphite is the first and only graphic design package to offer procedural vector editing.**

</div>
</section>

<section id="proceduralism-demo">
<div class="block">

Proceduralism lets you create sophisticated design elements that are easy to edit and reuse. The holiday string lights shown below are built from a simple group of nodes, allowing you to effortlessly reshape the wire and update the bulb appearance and spacing. <a href="https://editor.graphite.rs/#demo/procedural-string-lights">Click here to explore this demo</a> and try dragging the wire layer's points with the Path tool <span style="white-space: nowrap">(<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" style="vertical-align: middle"><polygon fill="#aaa" points="5,0 5,17 10,12 17,12" /><path fill="#78bae5" d="M20.77,14.36c-0.35-0.42-0.98-0.48-1.41-0.13c-1.04,0.87-2.19,1.6-3.36,2.24V16h-6v2.9c-2.88,0.84-5.07,1.1-5.11,1.11c-0.55,0.06-0.94,0.56-0.88,1.11C4.06,21.62,4.5,22,5,22c0.04,0,0.07,0,0.11-0.01c0.17-0.02,2.18-0.26,4.89-1.01V22h6v-3.28c1.6-0.79,3.2-1.75,4.64-2.95C21.06,15.42,21.12,14.78,20.77,14.36z M14,20h-2v-2h2V20z" /></svg>).</span>

<div class="video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play>
		<source src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/christmas-tree-lights.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/christmas-tree-lights.mp4" type="video/mp4" />
	</video>
</div>
<div class="demo-artwork">
	<a href="https://editor.graphite.rs/#demo/procedural-string-lights">
		<img src="https://static.graphite.rs/content/index/procedural-string-lights-thumbnail.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of Procedural String Lights" />
	</a>
	<p>
		<span>
			<em>Procedural String Lights</em>
		</span>
		<br />
		<span>
			<a href="https://editor.graphite.rs/#demo/procedural-string-lights">Open this artwork</a> to<br />explore it yourself.
		</span>
	</p>
</div>

</div>
</section>

<section id="proceduralism-features">
<div class="block">

Graphite's procedural, data-driven approach to graphic design affords unique capabilities *(while in alpha, these remain a work in progress)*:

<div class="feature-icons four-wide">
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 1" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Fully nondestructive editing with node-driven layers</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 9" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Infinitely scalable raster content with no pixelation</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Versatile modularity of node-based generative AI models</span>
	</div>
	<div class="feature-icon">
		<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Procedural pipelines for studio production environments</span>
	</div>
</div>

</div>
</section>
<!-- ▙ PROCEDURALISM ▟ -->
<!--                   -->
<!-- ▛ FUNDRAISING ▜ -->
<section id="fundraising" class="feature-box-outer">
<div class="feature-box-inner">

<h1 class="feature-box-header">Support the mission</h1>

---

<div class="block">

<p class="balance-text">
You can help realize Graphite's ambitious vision of building the ultimate 2D creative tool.
Graphite is built by a small, dedicated crew of volunteers in need of the resources to grow.
</p>

<a href="https://github.com/sponsors/GraphiteEditor" class="button arrow">Donate</a>

</div>

</div>
</section>
<!-- ▙ FUNDRAISING ▟ -->
<!--                 -->
<!-- ▛ VECTOR ART ▜ -->
<section id="vector-art">
<div class="block">

# Taking shape

**All you've come to expect from a professional vector graphics editor. Now readily accessible in your browser.**

<p>
<center>
Make vector art out of shapes ranging from simple geometric primitives to complex Bézier curves.
<br />
Style shapes with strokes, fills, and gradients. Mix layers with blend modes. Then export as an image or SVG.</center>
</p>

<div class="video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play>
		<source src="https://static.graphite.rs/content/index/just-a-potted-cactus-timelapse.webm" type="video/webm" />
		<source src="https://static.graphite.rs/content/index/just-a-potted-cactus-timelapse.mp4" type="video/mp4" />
	</video>
</div>
<div class="demo-artwork">
	<a href="https://editor.graphite.rs/#demo/just-a-potted-cactus">
		<img src="https://static.graphite.rs/content/index/just-a-potted-cactus-thumbnail.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of Just of Potted Cactus" />
	</a>
	<p>
		<span>
			<em>Just a Potted Cactus</em>
		</span>
		<br />
		<span>
			<a href="https://editor.graphite.rs/#demo/just-a-potted-cactus">Open this artwork</a> to<br />explore it yourself.
		</span>
	</p>
</div>

</div>
</section>
<!-- ▙ VECTOR ART ▟ -->
<!--                   -->
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
<!--                 -->
<!-- ▛ DEMO VIDEO ▜ -->
<section id="demo-video">
<div class="block">

Watch this timelapse showing the process of mixing traditional vector art (tracing a physical sketch and colorizing it, first two minutes) with using Imaginate to generate a background (last 45 seconds).

<div class="video-embed aspect-16x9">
	<img data-video-embed="JgJvAHQLnXA" src="https://static.graphite.rs/content/index/commander-basstronaut-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite - Vector Editing: &quot;Commander Basstronaut&quot; Artwork (25x Timelapse)" />
</div>

(Recorded in an older version of Graphite from early 2023.)

</div>
</section>
<!-- ▙ DEMO VIDEO ▟ -->
<!--                 -->
<!-- ▛ RECENT NEWS ▜ -->
<section id="recent-news" class="feature-box-outer">
	<div class="feature-box-inner">
		<h1 class="feature-box-header">Recent news <span> / </span> <a href="/blog" class="link arrow">More in the blog</a></h1>
		<hr />
		<div class="diptych">
		<!-- replacements::blog_posts(count = 2) -->
		</div>
	</div>
</section>
<!-- ▙ RECENT NEWS ▟ -->
