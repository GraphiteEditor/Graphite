# This is a helper file for people using NixOS as their operating system.
# If you don't know what this file does, you can safely ignore it.

# If you are using Nix as your package manager, you can run 'nix-shell .nix'
# in the root directory of the project and Nix will open a bash shell
# with all the packages needed to build and run Graphite installed.
# A shell.nix file is used in the Nix ecosystem to define a development
# environment with specific dependencies. When you enter a Nix shell using
# this file, it ensures that all the specified tools and libraries are
# available regardless of the host system's configuration. This provides
# a reproducible development environment across different machines and developers.

# You can enter the Nix shell and run Graphite like normal with:
# > npm start
# Or you can run it like this without needing to first enter the Nix shell:
# > nix-shell .nix --command "npm start"

# Uses flake compat to provide a development shell that is identical to the one defined in the flake
(import
  (
    let
      lock = builtins.fromJSON (builtins.readFile ./flake.lock);
      nodeName = lock.nodes.root.inputs.flake-compat;
    in
    fetchTarball {
      url = lock.nodes.${nodeName}.locked.url;
      sha256 = lock.nodes.${nodeName}.locked.narHash;
    }
  )
  { src = ./.; }
).shellNix
