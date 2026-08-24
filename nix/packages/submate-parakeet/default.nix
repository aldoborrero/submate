{ pkgs }:
# CPU build including the transcribe.cpp/Parakeet engine (`--engine parakeet`);
# whisper.cpp stays available as the default engine.
pkgs.callPackage ../submate/package.nix { parakeet = true; }
