{
  lib,
  rustPlatform,
  cmake,
  clang,
  gnumake,
  pkg-config,
}:

# The native-Rust port (rust/). Builds the `submate` CLI with the `model`
# feature, which compiles whisper.cpp via whisper-rs (needs cmake + a C/C++
# toolchain + libclang for bindgen). CPU-only here; GPU variants add the
# matching cargo feature + toolkit (see rust/README.md).
rustPlatform.buildRustPackage {
  pname = "submate-rs";
  version = "0.1.0";

  src = ../../../rust;
  cargoLock.lockFile = ../../../rust/Cargo.lock;

  nativeBuildInputs = [
    rustPlatform.bindgenHook # sets LIBCLANG_PATH + clang args for whisper-rs bindgen
    cmake # whisper-rs-sys builds whisper.cpp with cmake
    clang
    gnumake
    pkg-config
  ];

  # Build only the CLI binary, with real whisper.cpp inference.
  cargoBuildFlags = [
    "-p"
    "submate-cli"
    "--features"
    "model"
  ];

  # cmake's own configure inside the sandbox; don't let buildRustPackage's
  # cmake hook try to configure the Rust crate as a cmake project.
  dontUseCmakeConfigure = true;

  # Tests need a downloaded model + network; covered by the dev gate, not here.
  doCheck = false;

  meta = {
    description = "submate (Rust port): Whisper subtitle generation + LLM translation";
    homepage = "https://github.com/aldoborrero/submate";
    license = lib.licenses.mit;
    mainProgram = "submate";
  };
}
