# Deprecated alias for the Flake in packaging/. It keeps existing
# `github:MaxITService/MouseKeyStudio` inputs working while users move to
# `github:MaxITService/MouseKeyStudio?dir=packaging`; remove it in a later release.
#
# The alias declares nixpkgs only so `inputs.openlogi.inputs.nixpkgs.follows`
# still reaches the package. It has no committed flake.lock (see .gitignore):
# packaging/flake.lock pins every other input. The relative path input needs
# Nix 2.26 or newer.
{
  description = "Deprecated alias for github:MaxITService/MouseKeyStudio?dir=packaging";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    openlogi = {
      url = "path:./packaging";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { openlogi, ... }:
    let
      moved = "the MouseKeyStudio Flake moved: replace github:MaxITService/MouseKeyStudio with github:MaxITService/MouseKeyStudio?dir=packaging; the root alias will be removed in a later release";
    in
    builtins.warn moved openlogi.outputs;
}
