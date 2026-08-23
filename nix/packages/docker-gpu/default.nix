{ pkgs, perSystem }:
pkgs.callPackage ./package.nix {
  submate-cuda = perSystem.self.submate-cuda;
}
