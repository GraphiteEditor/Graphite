+++
title = "Uses and workflows"

[extra]
order = 2 # Page number after chapter intro
+++

**NOTE: This is old. Some parts may not match current usage.**

This list describes some long-term aspirational goals and ideas. It represents an incomplete brainstorm idea dump, not a roadmap.

## Use cases

General goals for product capabilities are categorized by discipline below.

### Photography
- RAW photo editing/processing
- Batch processing pipeline

### Motion graphics
- Title sequences/kinetic typography/etc.

### Live broadcast/streaming
- Live video compositing/overlays
- Interactive exhibits (rendering live content for museum/art/festival exhibits)

### Web
- SVG design
- Animated or interactive SVG design

### Automation
- Batch multimedia editing/conversion

### Graphic design
- Print design
- Web/digital-focused graphics (marketing, branding, infographics, ads, etc.)
- Templates filled with file/spreadsheet data (e.g. prototyping/iterating components of a board/card game)

### Illustration
- Digital painting
- Logo and icon design

### Desktop publishing
- Templates filled with Markdown/HTML content with export to PDF

### Video compositing

### Data Visualization
- Data-powered graphs/charts/etc.
- Automated rendering with live/often-updated data

### 3D/Gamedev
- PBR procedural material authorship
- 3D model UV map texturing

### AI-assisted tools

### HDR processing

### 360° and panoramic stitching and spherical editing

## User stories

Example user workflows are categorized by discipline below.

### Photography
- Using a face detection node to sort photos into the correct folders upon export
- Using a face detection node to place a watermark near every face to prevent customers from cropping out watermarks placed in less important areas of some photos
- Shadow removal from part of an image by masking the location of a shadow and using the texture details of the darker area and the lighting and color context of the illuminated area
- Lightening or contextually infilling all the areas where a camera sensor has dust specks
- Isolating clipping highlights (that have expanded into neighboring pixels) in one or more color channels from point light sources like city lights and rendering smoother bright gradient point lights in place of them
- Advanced, intuitive blending with a node that lets you create a "custom blend mode" by specifying something like "Anything that's white: display as is. Anything else further from white by luminosity: fade out the saturation and opacity."

### Image editing
- Removing translucent watermarks that were applied in the same location to a batch of photos by finding their shared similarities and differences and using that as a subtraction diff

### Game development
- Design a GUI for the game and use Graphene to render the in-game GUI textures at runtime at the desired resolution without scaling problems, or even render it live as data updates its state
- Authoring procedural noise-based textures and PBR materials

### Data visualization
- Creating a chart from a CSV
- Rendering an always-up-to-date chart powered by real-time updates from a database
- Data-driven infographics like an org chart that can be updated with text instead of manual design work
- Rendering a timelapse video of every operation done in the history of a document

### Digital painting
- Creating a digital acrylic or oil painting using various brushes
- Preventing mixing/smearing of previous wet paint layers by drying it with a hair dryer tool
- Smearing wet paint colors together on a simulated paint palette and then sampling paint colors from that palette to paint with

### Graphic design
- Prototyping cards for board games fed with data in a spreadsheet which generates the cards from a template
- Creating an image that has been shredded but pieced back together, where the image can be updated then return to the shredded one without having to redo the editing steps to shred it

### Broadcast, interactive exhibits, and digital signage
- Rendering overlays for live streams or television broadcasts based on live input data, for example somebody donates and leaves a comment on a live stream and this web hook could trigger an animated display containing the user and their comment, or live telemetry for a rocket launch streams in and gets rendered as graphical overlays for a webcast.
- Rendering a custom live clockface with hour/minute/second hands based on an input of the current time, then showing them fullscreen on a display
- Request the weather from an API and render live visualizations which gets displayed on a monitor in your house or a museum (export to a Windows screen saver?)
- Data from sensors can render interactive 2D graphics at a museum art installation
- A storefront can have a monitor set up showing daily or hourly sales items based on web hooks or polling from the company’s website
- Polling an API for content like Twitter or an RSS feed and displaying the tweet or headline when it arrives on screen, styled as desired

### Print and publishing
- Formatting Markdown documents into PDF print layouts
- Laying out book covers for proper PDF export to a printer
- Typesetting and formatting all the interior pages of a book with manual control where needed

### Automation
- Laser cutter artwork processing for automating custom Etsy orders
- Running on a server to let users upload images for a custom T-shirt printing website, and it renders their graphic on the model’s shirt (or other custom printing online stores)
- Generating a PDF invoice based on data in a pipeline on a server

### Computer vision and industrial control
- Factory line is examining its fruit for defects. In order to verify the quality, they need to enhance the contrast, automatically orient the image and correct the lighting. They then pass the results into a machine learning algorithm to classify, and sometimes need to snoop on the stream of data to manually do quality control (ImageMagick or custom Python scripts are often used for this right now)
