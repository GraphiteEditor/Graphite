+++
title = "Looking back on 2023 and what's next"
date = 2024-01-01

[extra]
banner = "https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next.avif"
banner_png = "https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next.png"
author = "Keavon Chambers"
reddit = "https://www.reddit.com/r/graphite/comments/18xmoti/blog_post_looking_back_on_2023_and_whats_next/"
twitter = "https://twitter.com/GraphiteEditor/status/1742576805532577937"

js = ["video-embed.js"]
+++

The new year is here, and with so many accomplishments to share from the past twelve months, let's revisit the highlights of 2023 for the Graphite project. Now that winter has entered, let's swing back to the spring, summarize the summer, and follow this fall's noteworthy developments that brought another year of fruitful progress to Graphite's mission of re-envisioning artists' 2D creative workflows with the best free software we can build for the open source community. This past year as a team, we all got closer— to one another from continents apart; to visiting and connecting with our industry peers; and to reaching exciting new development milestones.

<!-- more -->

I am grateful to everyone who has placed their faith in my vision for Graphite since I laid forth the design and wrote its first line of code nearly three years ago. Meeting some of the amazing people this summer who helped to make it possible, and inspired the project in the first place, was a pleasure and an honor. From California to Europe and back again, my combined family vacation and Graphite outreach tour was an opportunity to make connections with those helping us reach our goals. This blog post is both a project update and a public thank-you to those who generously lent their time and attention to our small-but-growing project. And for readers eager for an update on the software itself, stick around (or skip ahead) for a development progress report and a look at what's coming down the pipeline in the new year.

<div class="video-background" style="text-align: center">
	<video autoplay loop muted playsinline disablepictureinpicture disableremoteplayback>
		<source src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/christmas-tree-lights.mp4" type="video/mp4" />
	</video>
</div>

<center><em>Happy Holidays from the Graphite team!<br />These procedural light strands are powered by the newly completed node graph features.<br /><a href="https://editor.graphite.rs/#demo/procedural-string-lights">Click here to explore this demo</a> — drag the wire layer's points with the Path tool.</em></center>

## The Alpha 2 release series

February marked the start of our second year developing Graphite under the alpha release banner. We took the opportunity to declare the start of a new release series, Alpha 2, for the year's focus on integrating the node graph. That goal was a success, and we anticipate the next release series, Alpha 3, will begin next month in February with a focus on procedural art workflows in line with our [roadmap](/features#roadmap).

## GDC and meetups with the Rust graphics community

In March, I attended the Game Developers Conference (GDC) in nearby San Francisco to network with professionals in the creative industry. I was accompanied by [Oliver Davies](https://github.com/otdavies), a Graphite founding collaborator, fellow 3D artist and graphics engineer, and a life-long friend of mine. We connected with Francesco Siddi and Dalai Felinto from the Blender Foundation and introduced them to the project. The conference was also an opportunity to meet face-to-face with subject-matter experts with whom I'd earlier conversed online. Because we write Graphite in the Rust programming language, we attended an impromptu meetup among game developers using Rust in the nearby park that stretched for several insightful hours of pertinent conversation topics, thusly concluding the last day of the event.

Later, in May, I went to another Rust developers meetup here in the Bay Area together with [Leonard Pauli](https://twitter.com/leonardpauli), a Graphite community member and code contributor who was in town on a visit all the way from Sweden. The event was headlined with a [presentation](https://www.youtube.com/watch?v=XjbVnwBtVEk) by [Raph Levien](https://raphlinus.github.io/) about [Xilem](https://github.com/linebender/xilem), an under-development GUI toolkit that Graphite may adopt someday for its promise of powering native, speedy desktop user interfaces. Raph Levien is a researcher and expert in the fields of 2D vector graphics, GPU-accelerated rendering, and the mathematics of splines and curves— topics considerably overlapping with Graphite's own technical disciplines. The meetup was a nice face-to-face introduction before I'd end up seeing Raph and Leonard each again very soon.

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/bay-area-rust-meetup.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Raph Levien speaking about Xilem" /></p>

<center><em>Raph gives his talk about Xilem and GUIs in Rust</em></center>

The next week I accepted Raph's invitation to visit him at his employer, Google, in Mountain View where we spent several hours talking shop. His other flagship open source library, [Vello](https://github.com/linebender/vello), is the high-performance 2D vector graphics renderer we plan to use as a crucial part of Graphite's render pipeline. Our discussions dove into the history and goals of Graphite, our shared research challenges, and covered some fascinating details surrounding Vello, computational geometry, and GPU rendering. As we round out 2023, Graphite's roadmap is finally nearing the stage of integrating Vello in the coming weeks and I look forward to growing our collaboration with Raph and his [research group](https://linebender.org/).

## Embark Studios visit in Stockholm, Sweden

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/embark.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Leonard and Keavon in front of the Embark office sign" /></p>

<center><em>Leonard (left) and Keavon (right) at Embark Studios</em></center>

In June, as part of a vacation with my family to Europe, I caught up again with Leonard and he led us on a tour of his beautiful city of Stockholm. I also reached out to Johan Andersson, CTO of Embark Studios and an ambassador for the open source Rust developer community. Embark very generously contributes open source libraries for the Rust computer graphics ecosystem that are vital to Graphite, including [rust-gpu](https://github.com/EmbarkStudios/rust-gpu) which powers our node graph engine's compilation to GPU compute shaders. Johan showed Lenoard and me around Embark and we all chatted about what each of us are pursuing with Rust in the creative software industry. (By the way, Embark just released their first game, [The Finals](https://www.reachthefinals.com/)— check it out!)

## Blender visit in Amsterdam, The Netherlands

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/blender-hq.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Graphite and Blender team members in front of the Blender building" /></p>

<center><em>Left to right: Dalai, Dennis, Keavon, Ton, Francesco</em></center>

The next month in July, together with Graphite's lead engineer [Dennis Kobert](/about#dennis-kobert), we spent the afternoon visiting Blender's headquarters in Amsterdam. Blender has been, since the beginning, my inspiration and motivation for taking on the tremendously ambitious goal of building Graphite. As fellow open source software projects building digital content creation tools, this was a wonderful chance to see where the magic happens and meet the people behind the curtain.

At the invitation of Francesco Siddi, COO of the Blender Foundation whom I'd met earlier at GDC, Dennis and I presented a lunchtime talk for the staff to introduce the Graphite project. Ton Roosendaal, Blender's founder, kindly cooked up some scrumptious, lovingly-made meals of fried eggs for us and his team. We spent a couple hours mingling— answering and asking questions and chatting about design and technical topics ranging from our Rust node graph language infrastructure to Blender's experience with color science standards.

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/blender-presentation.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Keavon presents in front of a projector screen at Blender's offices" /></p>

<center><em>Keavon presents to the Blender staff</em></center>

Ton also very generously gave us his time and full attention with an office hour for Dennis and me to ask his advice and learn from the three decades of experience that led to Blender's decisive success. (On January 2, Blender turns 30! They're aiming for a goal of 10,000 supporters to [donate a birthday gift](https://fund.blender.org/). I encourage you to join me in doing so.)

## Graphite developer retreat in Karlsruhe, Germany

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/karlsruhe.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Keavon and Dennis in front of Karlsruhe Palace" /></p>

<center><em>Keavon (left) and Dennis (right) at Karlsruhe Palace</em></center>

The week before the Blender visit, I arrived in Karlsruhe, Germany for a two-week stay with Dennis for our first-ever team retreat. After spending hundreds of hours collaborating online, it was great to finally meet in-person. From diving deep into Graphite design discussions, to exploring the city and his college campus and joining his friends in social activities (and thus meeting another Graphite contributor, Isaac Körner, recruited by Dennis), it was a very welcoming and productive exchange.

## SIGGRAPH in Los Angeles, California

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/siggraph.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="Keavon and Oliver standing in front of the SIGGRAPH conference sign" /></p>

<center><em>Keavon (left) and Oliver (right) at SIGGRAPH</em></center>

After returning back home to the States, the following month in August I road tripped down south to Los Angeles for another conference visit with Oliver Davies. SIGGRAPH is the industry's largest computer graphics conference which presented numerous opportunities to network with others in the field. Blender also exhibits each year and I got to meet up again with the crew from Amsterdam and meet some new faces who were absent in our earlier visit. I look forward to returning to LA this coming April for the first full Blender Conference held in the US and connecting with more like-minded open source creative aficionados.

## Incorporating Graphite Labs, LLC

The next big news of August was my formation of [Graphite Labs, LLC](https://www.linkedin.com/company/graphite-labs) as a legal entity, allowing us to open a bank account and sign contracts on behalf of the Graphite project. This is an important step in the professional growth of the project. A tax-exempt nonprotfit foundation may happen in the future, but an LLC is a more accessible starting point. This step has opened up opportunities to form partnerships with industry, collect [sponsorships and donations](/donate), and hire full-time engineers a few years down the road once we have the income to financially support others developing Graphite. A major goal in 2024 is growing the sustainability and financial independence of the organization and allowing myself a modest income stream to offset costs while continuing my full-time Graphite work.

## Website, user manual, and tutorials

I allocated my time at several points throughout the year into growing and evolving this website with a refreshed and more visually-appealing home page, dedicated pages for information [about](/about) the project and its [features](/features), an area providing resources and help for [volunteers](/volunteer) and [code contributors](/volunteer/guide), and just this month— a [user manual](/learn) complete with an introductory tutorial series. The first video went up yesterday:

<div class="video-embed aspect-16x9">
	<img data-video-embed="7gjUhl_3X10" src="https://static.graphite.rs/content/learn/introduction/tutorial-1-vector-art-quickstart-youtube.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graphite Tutorial 1 - Hands-On Quickstart" />
</div>

The user manual and tutorial series will continue expanding throughout the coming weeks. Additional website features including user accounts, forums, and other community features are being planned.

## 2023 development progress report

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/commit-rate-graph.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Graph visualizing the number of code commits each week of 2023" /></p>

<center><em>Weekly code commit rate in 2023</em></center>

Starting out the year twelve months ago, Graphite's core vector editing tools were in good shape and the node graph engine had just reached its experimental beginnings as a way of applying color filters to bitmap images. Further developing the graph engine (called Graphene) and integrating it throughout every part of the Graphite editor thus became the focus for 2023.

While much of the team's time was spent on refactors to swap short-term placeholder code with Graphene-powered replacements, this one-step-back, two-steps-forward approach has ultimately led to exciting new capabilities for users to design procedurally-generated vector art. I'm aware of no other vector graphics editor with a node-based procedural editing workflow, so this makes me especially thrilled to release the first app of its kind with that unique and useful capability.

But 2023 wasn't only put towards refactoring code. New features were added all throughout the year and here are a few favorites.

- Graphene's node infrastructure has seen steady developments which significantly upgraded the power of the node graph, its performance, and its frontend usability.
- There are lots of new nodes that do neat things! From complex color adjustments like Vibrance to procedural building blocks like Copy to Points and noise pattern generators, there are plenty to try out (and so many more coming in 2024).
- The (still rudimentary) Brush tool was added for drawing simple raster-based sketches.
- Drawing custom vector shapes with the Pen and Path tools saw usability improvements with point selection, nudging/transformation, and entering exact numerical positions.
- Number input boxes in the UI can now be dragged to update their values and have math expressions evaluated automatically for convenience. Double a value just by typing `*2` at the end, or take the square root by wrapping it within `sqrt(` and `)`.
- A button to quickly open [pre-made sample art](https://editor.graphite.rs/#demo/valley-of-spires) documents was included at the suggestion of the fine folks at Blender. This helps new users see Graphite in action instead of just opening up an overwhelmingly barren blank canvas.

## Integrating the node graph

With so much to change in the goal of rewriting nearly every system with its Graphene counterpart, we had to take an incremental path so other feature development could continue without a broken editor. As succinctly as possible, this is the story of how we pulled it off— although this section gets rather technical so feel free to skip past if that's not your cup of tea.

The previous, intentionally-temporary layer system supported folders, vector shapes, text, bitmap images, and dynamic AI art (part of the Imaginate image generation feature).

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/migration-block-diagram-1.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Block diagram of a Graphite document before the node graph" /></p>

<center><em>Block diagram before the node graph</em></center>

To begin the incremental integration of nodes, we first added another type of layer, housing an instance of a Graphene node graph, which would supersede the other types. Then began the long process of porting all the other "legacy" layer types—and the viewport tools that operated on them—to become nodes. So the text layer type became a Text node managed by the Text tool, for example. This first phase was completed by April.

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/migration-block-diagram-2.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Block diagram of a Graphite document after the first phase of migration" /></p>

<center><em>Block diagram after the first phase</em></center>

Just folders and graph-driven legacy layers remained, but for the second phase, these (as well as artboards which were a separate temporary system) had to be combined into a single graph. Even with graph-based legacy layers, nodes couldn't yet interact between layer graphs to create interesting procedural designs. The Graphite vision calls for a single unified graph per document where all content lives, organized by layers, folders, and artboards collectively living within that graph. So phase two began with adding another node graph instance attached to the document itself instead of any particular layer. Then the old artboard system was replaced by artboard nodes in that graph, providing white backgrounds for the pages of artwork drawn atop by layers. Next, we built a new form of layer that would live in the node graph itself to provide organizational structure, acting as both a container for artwork nodes and a folder for other layers. By August, users could edit the document graph by hand but the viewport tools, folder hierarchy, rendering pipeline, and numerous other systems all still used the legacy layers.

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/migration-block-diagram-3.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Block diagram of a Graphite document after the second phase of migration" /></p>

<center><em>Block diagram after the second phase</em></center>

For the third phase, we had to yet again port each of the viewport tools so they would operate on the unified document graph instead of the legacy layer graphs. But the incremental approach ended here— this last phase had to happen all at once, which posed a challenge for developing the editor while numerous features were fully broken pending rework. We began with a separate development branch, always kept up-to-date with the latest editor code changes, for a couple months until breakages were reduced to an acceptable level, then integrated with the main code base in October. By tracking and burning down the list of [62 outstanding issues and regressions](https://github.com/GraphiteEditor/Graphite/issues/1394) one-by-one, in mid-December we finally reached our long-sought goal: deploying a new stable release of Graphite featuring the unified node graph! I'd like to extend an extra big thank-you to core team member ["Hypercube"](/about#hypercube) for the dedication and persistence in grinding through most of these.

<p><img src="https://static.graphite.rs/content/blog/2024-01-01-looking-back-on-2023-and-what's-next/migration-block-diagram-4.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Block diagram of a Graphite document now that the migration is complete" /></p>

<center><em>Block diagram of the completed integration</em></center>

I followed this up over my Christmas holiday by hunting down and ripping out over 6000 lines of unused code, satisfyingly bringing Graphite's total lines of Rust down to under 50,000— just about the same number as when we began, despite adding a year's worth of functionality! Fewer lines to understand and maintain makes our jobs easier, and I'm pleasantly surprised at how efficiently the Graphite team has managed to represent the app's considerable functionality in so relatively few lines. This is a sign of good engineering practices and it makes me proud of our capable team and what we have accomplished together.

## Looking ahead to 2024

The hard part is done. The unified document node graph, now that it's complete, paves the pathway towards new feature development for vector and raster editing. The Alpha 2 release series is nearly ready to become Alpha 3 with a focus towards procedural editing as the theme for the year ahead. And hopefully the road to the Beta release series, and then 1.0, is not too much further ahead.

First, I have some high-level goals for 2024:

- Cultivate a larger active community and grow the core team so we can move from a linear to an exponential pace of development
- Begin sending quarterly email [newsletters](/#newsletter), publish these blog posts more frequently, and find a dedicated volunteer to assist in writing them while also growing Graphite's social media and internet presence
- Announce Graphite to a wide audience and grow the daily active users by 10x or more, especially among artists
- Reach 20,000 [stars on GitHub](https://github.com/GraphiteEditor/Graphite/stargazers) (we just passed 5000 this December)
- Move towards a greater focus on polish, stability, performance, and learning resources for the product
- Attain sustainable income from donors and sponsors, and maybe even apply for grants in order to hire a full-time developer
- Build infrastructure for user accounts and prove the viability of getting revenue from hosted AI cloud computation

And then from a development perspective, I am looking forward to accomplishing these overarching objectives in 2024:

- Restoring several previous features that were removed during refactors in the past year to a fully working state including Imaginate, snapping, folder bounding boxes, transform pivots, and vector shape boolean operations
- Deploying GPU-based rendering by default and moving from an experimental to a production-ready hardware-accelerated compositing system using [Vello](https://github.com/linebender/vello) to unify the currently separate raster and vector pipelines
- Shipping desktop apps for Windows, Mac, and Linux by integrating [Tauri](https://tauri.app/) and bundling built-in AI models to run Imaginate and other upcoming features directly on user hardware
- Designing a new vector graphics data format suitable for advanced procedural editing and rendering, plus the associated procedural workflow features
- Remaking the Brush tool with the GPU-accelerated pipeline and the adaptive resolution system so digital painting in Graphite becomes practical
- Implementing the Mask Mode feature for Magic Wand tool marquee selections, which will dramatically improve Graphite's utility as a raster graphics editor
- Supporting animation capabilities (a potential stretch goal for the year)

## A call for community

Achieving everything listed above is ambitious, but it's ambition that has brought us to where we are today. Pulling this off will require a larger team and more resources than we've had in 2023. So if the mission we are striving for is exciting and you agree the world needs a truly great and versatile open source 2D graphics suite, we need your help!

- Technically inclined developers interested in Rust, web dev, computer graphics, backend programming, compilers, machine learning, mathematics, or any of the other varied disciplines that Graphite overlaps with— we likely have a role or project for you.
- We also have self-contained research projects involving problem-solving outside an existing code base. One example: there are numerous industry-standard image filter effects we'd like to implement in Graphite where a volunteer could run analysis on the colors of test images to identify a suitable algorithm that gives matching results. We hope to assemble a larger Discord community of motivated people we can tap for help in solving these sorts of problems. These also make great university term projects and we've mentored several groups successfully in the past, so please reach out.
- Technical artists with experience in procedural editing tools and engineers who enjoy designing solutions for complex problems would also be highly valuable community members when it comes to taking part in the many large, nuanced product and architecture design decisions we'll have to make this year.
- Graphic designers and artists who put the time into using Graphite on a regular basis and helping us learn its practical strengths and weaknesses would also be valuable contributors. Helping with the creative parts of maintaining the app and assisting new users who have questions in the growing community would relieve the burden from the core team.
- And spread the word! Create tutorial videos. Show off your creations on social media. Use it in your classroom. 2024 is the year Graphite is ready to come out of the shadows and get discovered.

If your New Year's resolution is joining an open source project, consider Graphite! We work hard to help new community contributors get up to speed with resources and guidance. We frequently hear praise that Graphite is a very inviting and supportive project from volunteers who have not had great prior experiences trying to get involved in open source. Join the Graphite [Discord server](https://discord.graphite.rs) and reach out to me (@Keavon) about how you'd like to get involved.

## Launching our supporter fund

There's one last big way you can help and keep Graphite from needing to turn to investors who would someday come knocking for exponential profits at the expense of you, the user. That doesn't align with my vision so I have been self-funding Graphite for the past three years. But I, alone, can't keep that going for a fourth year and beyond.

With a laser-tight focus on completing our 2023 development objectives, I haven't yet called out for donations until now. You can be the very first person to join at the level of a Supporter (starting at $10 monthly) or Sponsor (starting at $50 monthly for individuals and $100 monthly for companies). Please consider joining at one of the levels [listed here](https://github.com/sponsors/GraphiteEditor).

Please help launch Graphite towards the 2024 goal of attaining self-sufficiency so I can maintain my full-time commitment to an independent Graphite for the long haul. Thank you, it really means the world to me. ♥

<a href="/donate" class="button arrow">Become a supporter</a>

## Wrapping up

Thank you to our community for an incredible year. Passing [5000 stars](https://github.com/GraphiteEditor/Graphite/stargazers) on the project GitHub repository was a wonderful gift this holiday season. Everyone who has given advice, written code, and expressed enthusiasm has inspired me every day to stay focused and motivated. It was especially a pleasure and honor to visit the many people and organizations mentioned in this post throughout the past year. The reception by all has been heartwarming and I look forward to staying connected with them and a growing Graphite community in the promising year that now lies ahead. I am most of all excited for the awesome state Graphite will be in when it comes time to write this post again next year.

Now go [make some awesome art](https://editor.graphite.rs)!
