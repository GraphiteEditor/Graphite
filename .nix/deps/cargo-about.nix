# TODO: Remove once nixpkgs ships cargo-about >= 0.9.3, and switch back to `pkgs.cargo-about`
{ pkgs, ... }:

pkgs.cargo-about.overrideAttrs (
  finalAttrs: _: {
    version = "0.9.3";
    src = pkgs.fetchFromGitHub {
      owner = "EmbarkStudios";
      repo = "cargo-about";
      tag = finalAttrs.version;
      hash = "sha256-Jlr4TYNZkzYo5V77n0YYrx1e8m17ELN/1+ZcR/Bl0K4=";
    };
    cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
      inherit (finalAttrs) pname version src;
      hash = "sha256-kNGfQQumgvtDPhiQcqc9K2ohhEsTzuySeisHcNcmig8=";
    };
  }
)
