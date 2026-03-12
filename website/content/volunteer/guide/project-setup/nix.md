+++
title = "Nix setup"

[extra]
order = 2 # Page number after chapter intro
+++
## Using Graphite on NixOS/Nix

Run Graphite without installing:

```sh
nix run github:GraphiteEditor/Graphite
```

### Binary Cache

A [Binary Cache](https://graphite.cachix.org/) is available to avoid building from source.

You can use it directly on the command line:

```sh
nix run github:GraphiteEditor/Graphite --extra-substituters "https://graphite.cachix.org" --extra-trusted-public-keys "graphite.cachix.org-1:B7Il1yMpkquN/dXM+5GRmz+4Xmu2aaCS1GcWNfFhsOo="
```

### Using the Flake from the official repository

To add Graphite to a NixOS configuration, include the flake as an input:

```nix
{
  inputs = {
    nixpkgs.url = "...";
    graphite.url = "github:GraphiteEditor/Graphite";
  };

  outputs = { nixpkgs, graphite, ... }: {
    nixosConfigurations.your-host = nixpkgs.lib.nixosSystem {
      modules = [
        {
          environment.systemPackages = [
            graphite.packages.x86_64-linux.default
          ];

          nix.settings = {
            substituters = [ "https://graphite.cachix.org" ];
            trusted-public-keys = [
              "graphite.cachix.org-1:B7Il1yMpkquN/dXM+5GRmz+4Xmu2aaCS1GcWNfFhsOo="
            ];
          };
        }
      ];
    };
  };
}
```

### Nixpkgs

Graphite is also available in [nixpkgs](https://github.com/NixOS/nixpkgs) as [`graphite`](https://search.nixos.org/packages?channel=unstable&query=graphite&show=graphite).

All Graphite code is licensed under the [Apache License 2.0](https://graphite.art/license#source-code), but the derivation also bundles the official branding assets which are licensed under the separate [Graphite Branding License](https://graphite.art/license#branding). Graphite is therefore not considered free by nixpkgs and not cached in the main NixOS cache.

Pre-built binaries are instead available through the [nix-community cache](https://nix-community.org/cache/):

```nix
nix.settings = {
  substituters = [ "https://nix-community.cachix.org" ];
  trusted-public-keys = [
    "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
  ];
};
```
