{ pkgs, perSystem }:
pkgs.callPackage ./package.nix {
  submate-vulkan-parakeet = perSystem.self.submate-vulkan-parakeet;
}
