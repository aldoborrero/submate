{ pkgs, perSystem }:
pkgs.callPackage ./package.nix {
  submate = perSystem.self.submate;
}
