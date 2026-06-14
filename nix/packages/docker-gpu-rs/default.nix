{ pkgs, perSystem }:
pkgs.callPackage ./package.nix {
  submate-rs-cuda = perSystem.self.submate-rs-cuda;
}
