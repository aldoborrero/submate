{ pkgs, perSystem }:
pkgs.callPackage ./package.nix {
  submate-vulkan = perSystem.self.submate-vulkan;
}
