{ pkgs }:
# CUDA-accelerated build — NVIDIA GPUs. Needs allowUnfree (set in the flake).
pkgs.callPackage ../submate-rs/package.nix { gpuBackend = "cuda"; }
