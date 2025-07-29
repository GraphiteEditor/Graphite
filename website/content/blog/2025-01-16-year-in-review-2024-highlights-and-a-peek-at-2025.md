+++
title = "Year in review: 2024 highlights and a peek at 2025"
date = 2025-01-16

[extra]
banner = "https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025.avif"
banner_png = "https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025.png"
author = "Keavon Chambers"
summary = "Graphite has come a long way in 2024. Read about the progress made and the plans for the upcoming year."
reddit = "https://www.reddit.com/r/graphite/comments/1i3umnl/blog_post_year_in_review_2024_highlights_and_a/"
twitter = "https://x.com/GraphiteEditor/status/1880404337345851612"
bluesky = "https://bsky.app/profile/graphiteeditor.bsky.social/post/3lfxysayh622g"

js = ["/js/youtube-embed.js"]
css = ["/component/youtube-embed.css"]
+++

Another year has come and gone which has propelled Graphite—further than any year before—towards the ambitious goal of satiating the open source community's expanding appetite for an awesome 2D content creation suite that surpasses established choices in ease-of-use, powerful features, and affordability (at the unbeatable price of *free*).

<!-- more -->

In a world where the notion of software ownership seems headed towards extinction, the need has never been greater for an independent, community-built alternative to the vector graphics, animation, image manipulation, photo processing, and publishing tools used daily by millions of creators worldwide.

Graphite is and will always remain yours to keep, whether that's by running the lightweight, client-side [web app](https://editor.graphite.rs) (no signup, no cloud), <a href="https://support.google.com/chrome/answer/9658361" target="_blank">installing the PWA</a> on your desktop, self-hosting the <a href="https://github.com/GraphiteEditor/Graphite/releases/tag/latest-stable" target="_blank">builds</a>, or downloading the soon-to-be-ready native app for your OS of choice (more news on that later in the post).

<style class="float-image">
.float-image + p {
	text-align: left;
}
.float-image + p > a {
	float: right;
	margin-left: 1.5em;
	margin-bottom: 1em;
}
</style>

<p>
<a href="https://github.com/GraphiteEditor/Graphite"><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/10k-stars.avif" style="max-width: unset; margin-top: 0.5em" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Screenshot of 10,000 stars" /></a>
<span>Join me, founder and designer of Graphite, to see where the past year has brought us on this quest. And let me take this moment to thank our growing community for sharing my vision and showing support, both <a href="/donate">financially</a> and by boosting the GitHub project page over the 10,000 star milestone just in time to celebrate the end of a productive 2024.</span>
</p>

## 2024 development progress report

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/weekly-commit-rate.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" /></p>

<center><em>Weekly code commit rate in 2024</em></center>

In 2024, Graphite grew from a promising tech demo into a by-and-large useful application for vector artistry and graphic design, equipped with its totally unique secret ingredient: nondestructive, procedural editing via a node graph. This was a year focused on iterating until a prototype became a polished product— at least by the standards of alpha-stage software. If you haven't [tried Graphite](https://editor.graphite.rs) recently, please take another look!

Improvements made throughout Alpha 3 (2024's release series) brought the formerly abysmal performance up to now-adequate levels and solved the vast bulk of instability with the once-numerous crashes and bugs. Advancements to [Graphene](/volunteer/guide/graphene), our bespoke node graph engine technology, has let us begin to support new rendering possibilities, introduce more helpful nodes, and remove restrictive limitations with common node combinations that previously were points of frustration. We also made big strides improving the tools used by artists for vector drawing with features like boolean path operations, snapping, layer selection history, quick measurement, gradient picking, and extensive usability-focused tweaks—both big and small—all throughout the editor.

This was also the first year of publishing quarterly development reports to the blog. We have aimed to keep them all visually interesting by showing the new features with looping video clip demonstrations alongside digestible sentence-long change summaries. Please let us know if you find this format valuable and worth continuing in 2025.

- [Graphite progress report (Q1 2024)](../graphite-progress-report-q1-2024)
- [Graphite progress report (Q2 2024)](../graphite-progress-report-q2-2024)
- [Graphite progress report (Q3 2024)](../graphite-progress-report-q3-2024)
- Graphite progress report (Q4 2024) will be published soon— stay up-to-date with the [newsletter](/#newsletter) or [RSS feed](../rss.xml)

## Alpha roadmap update

Alpha 3 began back in February 2024 and we plan to declare the start of the Alpha 4 release series again in February after wrapping up the holiday development cycle and shifting gears for the big projects we plan to tackle in 2025. I'll expand on those goals for the upcoming year later in this article. If development keeps pace with plans, Alpha 4 may be the last before Beta begins as early as the start of 2026.

## Google Summer of Code results

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/gsoc-logo.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Google Summer of Code program logo" /></p>

<center><em>Read our short <a href="../graphite-internships-announcing-participation-in-gsoc-2024">blog post</a> about participating in Google Summer of Code</em></center>

Our project had the fortune of being selected to participate for its first year as a mentoring organization in <a href="https://summerofcode.withgoogle.com/archive/2024/organizations/graphite" target="_blank">Google Summer of Code</a> (GSoC). We were given the opportunity to welcome three student interns to the team as they developed significant contributions to our open source code base throughout the summer.

I would like to express my gratitude to Google for funding the program and its stipends given to our students. It has ushered in a great deal of talent to our contributor community. And even beginning right now, we are seeing proactive prospective students begin [submitting code in anticipation](../graphite-internships-announcing-participation-in-gsoc-2024) of the 2025 program. We hope our organization is invited to return for the opportunity to mentor another cohort of budding talent, much like our 2024 students—Adam, Elbert, and Dennis—whose accomplishments far surpassed my most optimistic expectations.

<a href="https://github.com/GraphiteEditor/Graphite/discussions/1769" target="_blank">Adam contributed</a> an extensive evolution of Graphite's node editor. He introduced adjustment layers, editable nested graphs, and layer/node organization features that have upgraded the capabilities of the procedural editing environment, bringing it closer to my product vision. I'm delighted to announce that Adam has decided to stick around past the summer and join the [core team](/about#core-team). His continued development of the node editing systems to fully realize my design goals will make the editor magnitudes more powerful, and at the same time, easier to use. More on those plans later in the article.

<a href="https://github.com/GraphiteEditor/Graphite/discussions/1771" target="_blank">Elbert built</a> a new Rust library, <a href="https://crates.io/crates/rawkit" target="_blank">Rawkit</a>, for decoding and processing `.arw` files from Sony digital cameras, one of the most popular camera brands. Nikon, Canon, and someday all other formats are in scope for the future direction of the code. This library gives us the fine-grained control we require and will allow Graphite to begin its focus on digital photo processing in 2025 once several technical limitations are overcome and Rawkit is integrated into Graphite.

<a href="https://github.com/GraphiteEditor/Graphite/discussions/1773" target="_blank">Dennis developed</a> a range of critical improvements around the theme of performance. Before his contributions, working in Graphite was too slow for practical usability. Afterwards, it became fast enough to be useful in most scenarios (except when dealing with excessive complexity in terms of pixel or vector point counts— those known bottlenecks remain as future work). He made rendering and various nodes such as boolean operations faster, built profiling tools to aid in the continued quest for speed, and made changes adding to the robustness of the entire node engine. Lastly, he integrated the <a href="https://github.com/linebender/vello" target="_blank">Vello</a> high-performance vector graphics renderer to replace our SVG-based rendering method. This streamlines the rasterization and compositing that's involved in showing you the artwork in your viewport. Vello can be turned on from the editor preferences menu and will be enabled by default later in 2025 when browser support for the WebGPU API, which Vello relies on, becomes widespread.

Another part of the GSoC program was the Mentor Summit in October where I got to meet and share knowledge with open source maintainers from the other participating organizations. Hosted at Google's Mountain View offices, it was a weekend of talking tech and learning the lessons of open source project management from the seasoned veterans.

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/gsoc-mentor-summit-photo.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>Group photo of the GSoC Mentor Summit attendees</em></center>

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/gsoc-mentor-summit-session-board.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>Schedule board highlighting my session on day 2</em></center>

But I also managed to pass on knowledge in a subject I'm passionate about: design. I led a session titled "Well-Designed User Interfaces in FOSS Apps" in which 30-40 participants came to explore and discuss how the open source community can improve in this notoriously challenging discipline. The Mentor Summit was just one of the community events we took part in over this past year.

## Community events

Outside of my daily routine spent coding, designing, and managing the project, I've also continued working to expand our community engagement and industry outreach. Members of the Graphite team met up at events during the year to represent the project and plant the seeds of future growth for our mission beyond the reach of the little pocket of the internet we call home.

### Game Developers Conference

In March, accompanied by Oliver Davies, a personal friend/tech artist/contributor to Graphite's product design, he and I visited the <a href="https://gdconf.com/" target="_blank">Game Developers Conference</a> (GDC) in San Francisco for opportunities to meet face-to-face with industry colleagues, as we did also the past two years.

We introduced Graphite to open source organizations like <a href="https://godotengine.org/" target="_blank">Godot</a> and <a href="https://o3de.org/" target="_blank">O3DE</a>, caught up with Francesco and Dalai from the Blender Foundation, joined a roundtable panel on open source adoption in the games industry, and came together with the <a href="https://gamedev.rs/" target="_blank">Rust Gamedev</a> community for a physical meetup of game and graphics centric Rust developers. It was a treat seeing several people in-person for the first time whom, before, I'd known only online.

<p class="wide"><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/gdc-collage.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>
Upper-left: Forest Anderson, host of the Rust Gamedev Meetup <a href="https://www.youtube.com/watch?v=Ea4Wt_FgEEw&list=PLYiOdhpKxxXI9l8V15FciLcsPzkz3Gt4I&index=2" target="_blank">live streams</a> where Graphite demoed many monthly development milestones, with me at the Rust devs hangout;
Upper-right: roundtable panel on open source adoption in the games industry;
Lower-left: Godot's presence at GDC;
Lower-right: Francesco Siddi (1) and Dalai Felinto (2) from the Blender Foundation with me (3) and Oliver (4)
</em></center>

With March now just around the corner, I am definitely looking forward to the next conference. If you will be in town for GDC 2025 and would like to meet up, please [get in touch](/contact).

### Blender Conference LA

<p class="wide"><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/bcon-la-collage.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>Left-to-right, top-then-bottom: Ton Roosendaal (founder of Blender) and me (founder of Graphite); the venue on Hollywood Boulevard; talks on the main stage; Colin Levy (director of <a href="https://www.youtube.com/watch?v=Mv30ExfoKcc" target="_blank">Skywatch</a> and the Blender Studio's <a href="https://www.youtube.com/watch?v=eRsGyueVLvQ" target="_blank">Sintel</a>) and Andrew Price (<a href="https://www.youtube.com/@blenderguru" target="_blank">Blender Guru</a> and creator of <a href="https://www.poliigon.com/" target="_blank">Poliigon</a>); my talk on the main stage; Alan Melikdjanian (<a href="https://www.youtube.com/user/CaptainDisillusion" target="_blank">Captain Disillusion</a>) and <a href="https://www.youtube.com/@IanHubert2" target="_blank">Ian Hubert</a> (YouTube filmmaker and director of the Blender Studio's <a href="https://www.youtube.com/watch?v=41hv2tW5Lc4" target="_blank">Tears of Steel</a>); attendees chatting</em></center>

The next month in April, Oliver and I went to our second conference of the year: <a href="https://bconla.org/2024/" target="_blank">BCON LA</a>, the first Blender Conference held in Los Angeles. We connected again with the Blender team and met many 3D/VFX industry professionals and prominent members of the Blender community over the two days of the event.

I also took to the main stage to present a lightning talk introducing Graphite to the community:

<div class="youtube-embed aspect-16x9">
	<img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/bcon-la-talk-video-cover.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" data-youtube-embed="x3P5eYv11EU" data-youtube-timestamp="1603" alt="BCON LA 2024 - Lightning Talks" />
</div>

### Graphite booth at Open Sauce

The month of June was particularly special because of <a href="https://opensauce.com/" target="_blank">Open Sauce</a>, a convention and expo in San Francisco for makers and creators, and of course, open source projects!

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/open-sauce-booth.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>Adam and Oliver driving a Graphite live demo at our booth</em></center>

This presented the perfect opportunity to host our own exhibitor booth and talk to hundreds of excited attendees with creative tech backgrounds over the two day show. The event was excellent for networking with fellow makers and a chance to meet an array of the guest YouTube creators including several from the digital content creation realm.

<p class="wide"><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/open-sauce-collage.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>Sides: doodles by attendees on our public art wall; Inner-left: <a href="https://www.youtube.com/@IanHubert2" target="_blank">Ian Hubert</a>, Blender filmmaker, visiting again after we met at BCON LA; Inner-right: Daniel Shiffman (<a href="https://www.youtube.com/@TheCodingTrain" target="_blank">The Coding Train</a>), creator of tutorials and explorations into creative coding/generative procedural art</em></center>

I designed the booth with the goal of becoming an inviting artist's space. Visitors could contribute doodles to the pair of LED-backlit dry erase boards, walk inside to talk with us about the project, and sit down to explore the app. This was a valuable chance to "playtest" the user experience with a steady supply of new people from a variety of backgrounds. I also learned how to refine our approach to communicating clearly what the product is and does.

Joining me again was Oliver to assist with the booth, as well as our new GSoC contributor, Adam, who flew up from southern California to help. In between the hustle and bustle, we put the face-to-face time to good use communicating the vision and planning many aspects of his node graph development.

### Graphite booth at the Bay Area Maker Faire

And then when October rolled around, we did it again! Now located at a post-industrial waterfront venue across the Bay from San Francisco, the <a href="https://makerfaire.com/bay-area/" target="_blank">Maker Faire</a> started with a Friday field trip day for local schools followed by a full weekend of general attendance.

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/maker-faire-booth.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>Visitors making art and learning about Graphite at our booth</em></center>

The Maker Faire is ground zero for the Maker Movement. I grew up going each year as a kid. It was an era when consumer 3D printing was completely new and that was the only place one could discover—and obsess over—the technology. The Faire influenced my career path into engineering and the arts, so it's fitting that I would grow up to return and share an open source project, born out of that community spirit, for the next generation of creative young minds.

Attracting a more family-oriented audience than Open Sauce, it presented the chance to learn how approachable Graphite is even for kids. Doodling on the LED-backlit whiteboards flanking our booth was especially popular with that age range and brought in many passers-by. Of those who tried Graphite, I was blown away to see how some of our youngest visitors—down to the age of 6—were also the most capable and engaged using the product, diving in deep with barely any instruction. As the UI and product designer, this assured me that I have been on the right track so far. I believe now more than ever that my ambitious goal is achievable: creating the most intuitive and user-friendly professional graphics editor on the market.

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/maker-faire-demo-stations.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>All four demo computers occupied with Graphite's newest users</em></center>

Another exciting part of the Maker Faire experience was bringing together nearly the full Graphite core team in-person. Oliver and Adam came to help again while we were also joined by Dennis visiting all the way from Germany, conveniently coinciding with a vacation he had planned. We put our commute time towards deeply technical code architecture discussions and knowledge transfer.

<p class="wide"><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/maker-faire-team-collage.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.jpg')" alt="" /></p>

<center><em>Left: me (1), Adam (2), and Oliver (3) at the venue; Right: Adam (1), Dennis (2), and me (3) (Oliver and Dennis couldn't make it on the same day)</em></center>

Graphite is tentatively anticipating a return for Open Sauce and the Maker Faire again in 2025. Make plans to come and visit!

## Looking ahead to 2025

There are so many plans that I'm eager to carry out to improve the clarity and capability of the experience Graphite offers our users. Some are small, and a few we will be working on for most of the year.

### Desktop app

Starting out with our most in-demand request: a desktop app. This has been on our roadmap from the start but only recently it's begun making sense putting it at the front of the roadmap priorities. We hoped to complete it by the end of 2024, but that wasn't in the cards due to developer availability and the specialized skills needed to complete the task.

Now to get technical, the lazy option exists: chucking the whole web app in an unaltered Electron wrapper, but this is a technological dead end that I believe offers no value compared to a PWA. The value comes from offering an actual native app where the editor Rust code and GPU-accelerated rendering runs on a user's Windows, Mac, or Linux machine without browser overhead. Our use case of combining the web-rendered editor interface with the user's native-rendered artwork presents several unique challenges that our team has to overcome— individually on each platform. If you have experience with native development on Windows, Mac, and/or Linux, please get involved to speed up this effort! With our current resources, I am anticipating this will be ready for release around spring.

### Animation

Next, the feature I am personally most itching to dive into developing is animation. I've been recently iterating on the UI design mockup for the Timeline panel which will support keyframing any desired node parameters. This new panel will seamlessly integrate into the existing graph-based, data-driven workflow and make it easy to create motion graphics paired with procedural generation.

<p class="wide"><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/timeline-panel-ui-mockup.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" /></p>

<center><em>User interface mockup for the latest animation panel plans prior to being implemented</em></center>

### Advanced procedural editing

It is now becoming time to delve into the next phase of making the node graph more powerful by introducing several key features:

- Lambdas: treating a node as a piece of data given to another node, so it can be run in a loop with varying parameters in each iteration.
- Instances: generalizing graphical data, transforms, and groups so that every layer is one or multiple instances, each with a unique transform. This will finally fix the long-lived limitation of layers lacking a proper pivot point.
- Tables: representing lists of data like vector points and segments in a spreadsheet. Formalizing the tabular data representation lets the node engine benefit from ECS-like performance gains by optimizing CPU cache utilization.
- Attributes: encoding properties (of points, of segments, of instances, of appearance styles, etc.) in columns on the tabular data. This will unlock Graphite to become as powerful as Blender geometry nodes which works based on the same design principle.

### Raster graphics editing

Graphite has included a primitive kind of raster support for a while, mostly used for including reference images when creating vector content. Some raster nodes can adjust the colors of images, but there are no tools yet for selecting and drawing over parts of an image to make localized edits. Furthermore, CPU-centric bottlenecks slow down the editor when big images are in use. The GPU is not used by any nodes operating on pixel-based data.

Consequently, raster editing just isn't viable yet until tool and GPU node support arrives. The innumerable complexities would not fit in this section, but if you have a background in compilers or graphics programming, please hop on <a href="https://discord.graphite.rs" target="_blank">our Discord</a> and ask about it if you're curious or potentially interested in helping. After the infrastructure parts are in place, we can begin building nodes that make localized edits (such as a masking node) and start developing tools including a fully rewritten brush engine and a mode for drawing marquee selections. I can't yet predict how far we will get by year's end with these interactive localized editing tools because it will all depend on how quickly the prerequisite technical infrastructure components come together. Part of that will depend on how soon the browser vendors ship universal <a href="https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API#browser_compatibility" target="_blank">WebGPU API support</a> so it can be deployed beyond an experimental state in Graphite.

### Graphite in education

A valuable discovery came out of exhibiting Graphite at the Maker Faire. I had conversations with several school teachers who were interested in using Graphite in their classrooms.

I was told that other web-based graphics editors are commonly blocked by the IT admins of school networks due to policies against visiting sites with ads. (Crazy!) Since Graphite is entirely ad-free and runs on Chromebooks, this presents an opportunity to focus on better supporting the education market. In fact, I have recently been watching Chromebooks account for a small but rapidly growing portion of our site's visitors which means some educators are teaching Graphite in their classrooms:

<p><img src="https://static.graphite.rs/content/blog/2025-01-16-year-in-review-2024-highlights-and-a-peek-at-2025/school-usage-trends.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="" /></p>

<center><em>Graphite visitors on Chromebooks have trended up this school year but dropped down to background levels during summer and winter breaks, hinting at usage in classrooms</em></center>

This was previously a user demographic that I didn't consider. My school experience never offered graphic design instruction. I had to teach it to myself, and when using school computers, I ran portable installations of remote desktop software to access the desktop creative applications on my home computer. That experience led to my selection of a software stack that would support web-based access to the Graphite editor for the sake of students like my past self, but my previous assumption was that teacher-led instruction would be years away.

Learning that there are educators who want to teach these skills directly to students was eye-opening. It could be an excellent fit: each teacher brings along dozens of users. A teacher can learn the app once and disseminate the instruction which saves us from needing pristine learning resources at this early stage. Then a teacher can find the most common sources of confusion and filter that feedback back to us for improvement. Compared to professional artists who can't always justify using alpha software, students are a less demanding type of user. And Graphite benefits from a generation of students growing up to continue using the app. It seems like a surprisingly good fit.

My goal in 2025 is to begin prioritizing specific resources for educators that might include:

- Pushing for development efforts that will improve performance to help the app run better on low-spec hardware. This benefits everyone else just as greatly!
- Reducing common pitfalls in the software that are especially likely to be encountered by inexperienced users.
- Putting more time towards creating learning resources and documentation to help instructors learn Graphite well enough to teach it and solve student issues.
- Collaborating with teachers to devise and develop a curriculum package that can be used in classrooms to teach specific skills.
- Creating an information page for educators to discover the project, learn how it suits their needs, access curriculum, and get connected with us.

If you are a teacher, or know one, who would be interested in adding this manner of STEM/STEAM instruction to your classroom, please [get in touch](/contact) so we can figure this out together.

## Thank you for helping us help you help us all

By not being backed by investors or built by staff engineers or marketed by an agency, Graphite is an ambition that is constantly treading the line that borders the realm of impossibility. But thanks to your support, I am confident our efforts will prevail. 2025 is the year when all the pieces fall into place with a desktop app, competitive performance, features, and raster image editing. It will be the free software you can own (and love) that holds up against the software you have no choice but to rent (and put up with as it trains AI on your private work).

Ultimately, reaching critical mass might take one year. It might take five. That part is up to you. Momentum and resources are both scarce which means you, personally, have the opportunity to make an outsized impact.

If you choose to [become a member](/donate), you are directly helping fund our expenses like conference travel and ordering T-shirts that keeps the volunteer team happy and motivated to code. Remember that "free software" doesn't mean it's free to produce, it just means someone else is paying for it if you aren't pitching in. We just added an option for [donating directly](/donate#supporter-memberships) without needing a GitHub account, so now it's easier than ever to contribute.

If you choose to [volunteer](/volunteer), you lift our greatest bottleneck—time—and bring your unique skills to the table. There are opportunities from coding to technical writing to art, design, and marketing. It's a team effort, but only if there's a team to delegate the efforts to.

And there are other ways to help out. Sign up as a QA tester in our <a href="https://discord.graphite.rs" target="_blank">Discord</a>. Make it your mission to share Graphite by word-of-mouth on the forums and online communities you frequent. Put it on the radar of the creators you follow. Create and post your own tutorials on the web. Use it regularly and share your creations in our Discord community and by tagging #Made<wbr />With<wbr />Graphite on social media.

## Upcoming: FOSDEM '25

If you'll be in Brussels, Belgium for the FOSDEM conference in several weeks (February 1–2), be sure to [reach out](/contact) and arrange a plan to meet up, chat, and pick up some Graphite stickers.

## Until next year

Thank you for dedicating the time to read about this latest annual collection of project updates. It has been a privilege leading this community and endeavor since 2021. As we wrap up four years of hard work and venture into the beginnings of a fifth, I am more eager than ever for the adventures that lie in wait during the times ahead.

Happy creating!
