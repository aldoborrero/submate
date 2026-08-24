{ pkgs }:
# Vulkan-accelerated build with BOTH whisper.cpp and the transcribe.cpp/Parakeet
# engine; select at runtime with `submate transcribe --engine whisper|parakeet`.
pkgs.callPackage ../submate/package.nix {
  gpuBackend = "vulkan";
  parakeet = true;
}
