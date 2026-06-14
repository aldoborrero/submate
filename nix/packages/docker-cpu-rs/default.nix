{ pkgs, perSystem }:
pkgs.callPackage ./package.nix {
  submate-rs = perSystem.self.submate-rs;
}
