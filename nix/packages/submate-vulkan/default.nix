{ pkgs }:
# Vulkan-accelerated build — cross-vendor GPU (incl. Intel iGPU).
pkgs.callPackage ../submate/package.nix { gpuBackend = "vulkan"; }
