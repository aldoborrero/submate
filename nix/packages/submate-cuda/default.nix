{ pkgs }:
# CUDA-accelerated build — NVIDIA GPUs. Needs allowUnfree (set in the flake).
pkgs.callPackage ../submate/package.nix { gpuBackend = "cuda"; }
