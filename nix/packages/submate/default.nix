{ pkgs, perSystem }:
pkgs.python313Packages.callPackage ./package.nix {
  stable-ts = perSystem.self.stable-ts;
}
