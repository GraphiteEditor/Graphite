# CLI headless client

This will be the official command line program for editing and rendering Graphite documents and assets. It acts as a stateless tool for calling the APIs of the document core library and the render engine core library. This is useful for automation through shell scripts, for example, rather than needing to write custom software to use the Graphite core libraries. Because the Graphite CLI tool does not rely on the editor core library, there is no concept of tools or state.
