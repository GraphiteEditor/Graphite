{ pkgs, inputs, ... }:

{
  lib = inputs.crane.mkLib pkgs;
}
