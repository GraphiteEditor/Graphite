+++
title = "Web-based vector graphics editor and design tool"
template = "section.html"

[extra]
css = ["index.css"]
js = ["image-interaction.js", "video-embed.js"]
+++

<!-- ▛ LOGO ▜ -->
<section id="logo">
	<img src="https://static.graphite.rs/logos/graphite-logotype-color.svg" alt="Graphite Logo" />
</section>
<!-- ▙ LOGO ▟ -->

<img class="pencil-texture" src="https://static.graphite.rs/textures/pencil-texture.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" />

<!-- ▛ QUICK LINKS ▜ -->
<section id="quick-links">
	<div>
		<a href="#community" class="button arrow">Subscribe to the newsletter</a>
		<a href="/donate" class="button arrow">&hearts; Support the mission</a>
	</div>
	<div>
		<a href="https://github.com/GraphiteEditor/Graphite" target="_blank">
			<img src="https://static.graphite.rs/icons/github.svg" alt="GitHub" />
		</a>
		<a href="https://www.reddit.com/r/graphite/" target="_blank">
			<img src="https://static.graphite.rs/icons/reddit.svg" alt="Reddit" />
		</a>
		<a href="https://twitter.com/graphiteeditor" target="_blank">
			<img src="https://static.graphite.rs/icons/twitter.svg" alt="Twitter" />
		</a>
		<a href="https://www.youtube.com/@GraphiteEditor" target="_blank">
			<img src="https://static.graphite.rs/icons/youtube.svg" alt="YouTube" />
		</a>
		<a href="https://discord.graphite.rs" target="_blank">
			<img src="https://static.graphite.rs/icons/discord.svg" alt="Discord" />
		</a>
	</div>
</section>
<!-- ▙ QUICK LINKS ▟ -->
<!--                -->
<!-- ▛ TAGLINE ▜ -->
<section id="tagline">

<h1 class="balance-text">Redefining state&#8209;of&#8209;the&#8209;art graphics editing</h1>

<p class="balance-text"><strong>Graphite</strong> is an in-development raster and vector graphics package that's free and open source. It is powered by a node graph compositing engine that fuses layers with nodes, providing a fully nondestructive editing experience.</p>

</section>
<!-- ▙ TAGLINE ▟ -->

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
	<div class="carousel-slide">
		<img src="https://static.graphite.rs/content/index/gui-demo-valley-of-spires__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #1" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #2" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-viewport__4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite UI image #3" data-carousel-image />
	</div>
	<div class="carousel-slide torn left">
		<img src="https://static.graphite.rs/content/index/gui-demo-valley-of-spires__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-viewport__4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" data-carousel-image />
	</div>
	<div class="carousel-slide torn right">
		<img src="https://static.graphite.rs/content/index/gui-demo-valley-of-spires__3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-nodes__4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" data-carousel-image />
		<img src="https://static.graphite.rs/content/index/gui-mockup-viewport__4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" data-carousel-image />
	</div>
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
			<button class="direction next" data-carousel-next>
				<svg width="40" height="40" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
					<path d="M20,0C8.95,0,0,8.95,0,20c0,11.05,8.95,20,20,20c11.05,0,20-8.95,20-20C40,8.95,31.05,0,20,0z M20,38c-9.93,0-18-8.07-18-18S10.07,2,20,2s18,8.07,18,18S29.93,38,20,38z" />
					<polygon points="16.71,9.29 15.29,10.71 24.59,20 15.29,29.29 16.71,30.71 27.41,20" />
				</svg>
			</button>
		</div>
		<div class="screenshot-description">
			<p class="active" data-carousel-description>
				<em>Valley of Spires</em> — <a href="https://editor.graphite.rs/#demo/valley-of-spires">Open this artwork</a> to explore it yourself.
			</p>
			<p data-carousel-description>
				Design mockup for the work-in-progress raster editing pipeline. Some nodes shown here are not implemented yet, but will be soon!
			</p>
			<p data-carousel-description>
				Design mockup for the work-in-progress raster editing workflow. Some viewport features shown here are not implemented yet, but will be soon!
			</p>
		</div>
	</div>
</section>

<!-- ▙ SCREENSHOTS ▟ -->
<!--                      -->
<!-- ▛ TODAY AND TOMORROW ▜ -->
<section id="today-and-tomorrow">
<div class="diptych">

<div class="section">

# Graphite today

<div class="informational-group features">
	<div class="informational">
		<img class="atlas" style="--atlas-index: 0" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Vector graphics editing</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 1" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Node-based layers</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 2" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>AI-assisted art creation</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 3" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Open source and free forever</span>
	</div>
</div>

Graphite is a lightweight vector graphics editor that runs offline in your browser (no sign up required) and offers the unique feature of a node-driven procedural vector workflow.

</div>
<div class="section">

# Graphite tomorrow

<div class="informational-group features">
	<div class="informational">
		<img class="atlas" style="--atlas-index: 4" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Looks and feels like traditional <span style="text-decoration: underline dotted; text-decoration-color: #16323f77;" title="&quot;what you see is what you get&quot;">WYSIWYG</span> design apps</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 5" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Clean, intuitive interface built by designers, for designers</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 6" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Real-time collaborative editing</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 7" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Windows/Mac/Linux/Web/iPad</span>
	</div>
</div>

<a href="/features#roadmap" class="button arrow">Roadmap</a>

</div>

</div>
</section>
<!-- ▙ TODAY AND TOMORROW ▟ -->
<!--                     -->
<!-- ▛ COMMUNITY ▜ -->
<section id="community" class="feature-box">
<div class="box">
<div class="diptych">

<div id="newsletter" class="section">

# Stay in the loop

Subscribe to the newsletter for quarterly updates on major development progress.

<div id="newsletter-success">

## Thanks!

You'll receive your first newsletter email with the next major Graphite news.

</div>

<form action="https://graphite.rs/newsletter-signup" method="post">
	<div class="same-line">
		<div class="column name">
			<label for="newsletter-name">First + last name:</label>
			<input id="newsletter-name" name="name" type="text" required />
		</div>
		<div class="column phone">
			<label for="newsletter-phone">Phone:</label>
			<input id="newsletter-phone" name="phone" type="text" tabindex="-1" autocomplete="off" />
		</div>
		<div class="column email">
			<label for="newsletter-email">Email address:</label>
			<input id="newsletter-email" name="email" type="email" required />
		</div>
	</div>
	<div class="column submit">
		<input type="submit" value="Subscribe" class="button" />
	</div>
</form>

</div>
<div id="social" class="section">

# Follow along

<div class="social-links">
	<div class="column">
		<a href="https://discord.graphite.rs" target="_blank">
			<img src="https://static.graphite.rs/icons/discord.svg" alt="Discord" />
			<span class="link arrow">Join on Discord</span>
		</a>
		<a href="https://www.reddit.com/r/graphite/" target="_blank">
			<img src="https://static.graphite.rs/icons/reddit.svg" alt="Reddit" />
			<span class="link not-uppercase arrow">/r/Graphite</span>
		</a>
	</div>
	<div class="column">
		<a href="https://github.com/GraphiteEditor/Graphite" target="_blank">
			<img src="https://static.graphite.rs/icons/github.svg" alt="GitHub" />
			<span class="link arrow">Star on GitHub</span>
		</a>
		<a href="https://twitter.com/graphiteeditor" target="_blank">
			<img src="https://static.graphite.rs/icons/twitter.svg" alt="Twitter" />
			<span class="link not-uppercase arrow">@GraphiteEditor</span>
		</a>
	</div>
</div>

</div>

</div>
</div>
</section>
<!-- ▙ COMMUNITY ▟ -->
<!--                 -->
<!-- ▛ PROCEDURALISM ▜ -->
<section id="proceduralism">
<div class="section">

# Powerful proceduralism

Graphite's data-driven approach to graphic design affords some unique capabilities (while in alpha, these remain a work in progress):

<div class="informational-group features four-wide">
	<div class="informational">
		<img class="atlas" style="--atlas-index: 8" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Fully nondestructive editing with node-driven layers</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 9" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Infinitely scalable raster content with no pixelation</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 10" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Integrated generative AI models and graphics algorithms</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 11" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span class="balance-text">Procedural pipelines for studio production environments</span>
	</div>
</div>

</div>
</section>
<!-- ▙ PROCEDURALISM ▟ -->
<!--                   -->
<!-- ▛ DISCIPLINES ▜ -->
<section id="disciplines">
<div class="section">

# One app to rule them all

Stop jumping between programs. Planned features will make Graphite a first-class design tool for these disciplines (listed by priority):

<div class="informational-group concepts">
	<div class="informational">
		<img class="atlas" style="--atlas-index: 12" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Graphic Design</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 13" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Image Editing</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 14" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Digital Painting</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 15" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Desktop Publishing</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 16" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>VFX Compositing</span>
	</div>
	<div class="informational">
		<img class="atlas" style="--atlas-index: 17" src="https://static.graphite.rs/icons/icon-atlas-features.png" alt="" />
		<span>Motion Graphics</span>
	</div>
</div>

</div>
</section>
<!-- ▙ DISCIPLINES ▟ -->
<!--                  -->
<!-- ▛ FUNDRAISING ▜ -->
<section id="fundraising" class="feature-box">
<div class="box">
<div>

<div class="section">

# Support the mission

<p class="balance-text">
You can help realize Graphite's ambitious vision of building the ultimate 2D creative tool.
Graphite is built by a small, dedicated crew of volunteers in need of resources to grow.
</p>

<!-- [Re-include the import for `"fundraising.js"` when re-enabling this.]

### Summer 2023 fundraising goal:

<div class="fundraising loading" data-fundraising>
	<div class="fundraising-bar" data-fundraising-bar style="--fundraising-percent: 0%">
		<div class="fundraising-bar-progress"></div>
	</div>
	<div class="goal-metrics">
		<span data-fundraising-percent>Progress: <span data-dynamic>0</span>%</span>
		<span data-fundraising-goal>Goal: $<span data-dynamic>0</span>/month</span>
	</div>
</div>

[Become a monthly supporter](https://github.com/sponsors/GraphiteEditor) this summer to collect an exclusive 💚 badge. Each season you support, a new heart design is yours to keep. In the future, they'll be shown on Graphite account profiles and community areas like forums and in-app collaboration. -->

<a href="https://github.com/sponsors/GraphiteEditor" class="button arrow">Donate</a>

</div>

<!-- <div class="graphic">
	<a href="https://github.com/sponsors/GraphiteEditor"><img src="https://files.keavon.com/-/OtherDroopyBoto/Spring_Heart.png" /></a>
</div> -->

</div>
</div>
</section>
<!-- ▙ FUNDRAISING ▟ -->
<!--                 -->
<!-- ▛ VECTOR ART ▜ -->
<section id="vector-art">
<div class="section">

# Art takes shape

Make vector art out of shapes ranging from simple geometric primitives to complex Bézier curves.

Style your shapes with strokes, fills, and gradients. Mix your layers with blend modes. Then export as SVG.

<div class="video-background">
	<video loop muted playsinline disablepictureinpicture disableremoteplayback data-auto-play>
		<source src="https://static.graphite.rs/content/index/just-a-potted-cactus-timelapse.mp4" type="video/mp4" />
	</video>
</div>
<div class="demo-artwork">
	<a href="https://editor.graphite.rs/#demo/just-a-potted-cactus">
		<img src="https://static.graphite.rs/content/index/just-a-potted-cactus-thumbnail.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Vector art of Just of Potted Cactus" />
	</a>
	<p>
		<span class="balance-text">
			<em>Just a Potted Cactus</em>
		</span>
		<br />
		<span class="balance-text">
			<a href="https://editor.graphite.rs/#demo/just-a-potted-cactus">Open this artwork</a> to explore it yourself.
		</span>
	</p>
</div>

</div>
</section>
<!-- ▙ VECTOR ART ▟ -->
<!--                   -->
<!-- ▛ IMAGINATE ▜ -->
<section id="imaginate">

<div class="section">

<h1><span class="alternating-text"><span>Co-create</span><span>Ideate</span><span>Illustrate</span><span>Generate</span><span>Iterate</span></span> with Imaginate</h1>

**Imaginate** is a node powered by <a href="https://en.wikipedia.org/wiki/Stable_Diffusion" target="_blank">Stable Diffusion</a> that makes AI-assisted art creation an easy, nondestructive process.
<!-- [Learn how](/learn/node-graph/imaginate) it works. -->

</div>
<div class="diptych">

<div class="section">

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
<div class="section">

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
<!-- ▙ IMAGINATE ▟ -->
<!--                 -->
<!-- ▛ DEMO VIDEO ▜ -->
<section id="demo-video">

<div class="section">

Watch this timelapse showing the process of mixing traditional vector art (tracing a physical sketch and colorizing it, first two minutes) with using Imaginate to generate a background (last 45 seconds).

<div class="video-embed aspect-16x9">
	<img data-video-embed="JgJvAHQLnXA" src="https://static.graphite.rs/content/index/commander-basstronaut-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite - Vector Editing: &quot;Commander Basstronaut&quot; Artwork (25x Timelapse)" />
</div>

(Recorded in an older version of Graphite from early 2023.)

</div>

</section>
<!-- ▙ DEMO VIDEO ▟ -->
<!--                   -->
<!-- ▛ GET INVOLVED ▜ -->
<section id="get-involved-box" class="feature-box">
<div class="box">
<div class="diptych">

<div class="section">

# Get involved

<p class="balance-text">The Graphite project could not exist without its community. Building its ambitious and versatile feature set will require contributions from artists, designers, developers, technical experts, and eagle-eyed bug hunters. Help build the future of digital art.</p>

<a href="/volunteer" class="button arrow">Volunteer</a>

</div>
<div class="graphic">
	<img src="https://static.graphite.rs/content/index/volunteer.svg" alt="" />
</div>

</div>
</div>
</section>
<!-- ▙ GET INVOLVED ▟ -->
