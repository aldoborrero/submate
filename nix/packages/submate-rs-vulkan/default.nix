{ pkgs }:
# Vulkan-accelerated build — cross-vendor GPU (incl. Intel iGPU).
pkgs.callPackage ../submate-rs/package.nix { gpuBackend = "vulkan"; }
