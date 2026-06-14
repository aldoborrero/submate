{
  lib,
  rustPlatform,
  cmake,
  clang,
  gnumake,
  pkg-config,
  cudaPackages,
  shaderc,
  vulkan-headers,
  vulkan-loader,
  # GPU backend: null (CPU), "cuda", or "vulkan". The variant packages set it;
  # it selects the matching cargo feature + adds that backend's build inputs.
  gpuBackend ? null,
}:

let
  isCuda = gpuBackend == "cuda";
  isVulkan = gpuBackend == "vulkan";
  feature = if gpuBackend == null then "model" else gpuBackend;
in
# The native-Rust port (rust/). Builds the `submate` CLI; the `model` feature
# compiles whisper.cpp via whisper-rs (needs cmake + a C/C++ toolchain + libclang
# for bindgen). A GPU backend adds its toolkit + the matching cargo feature.
rustPlatform.buildRustPackage {
  pname = "submate-rs" + lib.optionalString (gpuBackend != null) "-${gpuBackend}";
  version = "0.1.0";

  src = ../../../rust;
  cargoLock.lockFile = ../../../rust/Cargo.lock;

  nativeBuildInputs = [
    rustPlatform.bindgenHook # LIBCLANG_PATH + clang args for whisper-rs bindgen
    cmake # whisper-rs-sys builds whisper.cpp with cmake
    clang
    gnumake
    pkg-config
  ]
  ++ lib.optionals isCuda [ cudaPackages.cuda_nvcc ]
  ++ lib.optionals isVulkan [ shaderc ]; # glslc, to compile the Vulkan shaders

  buildInputs =
    lib.optionals isCuda [
      cudaPackages.cuda_cudart
      cudaPackages.libcublas
    ]
    ++ lib.optionals isVulkan [
      vulkan-headers
      vulkan-loader
    ];

  # Build only the CLI binary, with whisper.cpp inference (+ GPU backend).
  cargoBuildFlags = [
    "-p"
    "submate-cli"
    "--features"
    feature
  ];

  # whisper-rs-sys runs cmake itself; don't let the cmake hook try to configure
  # the Rust crate as a cmake project.
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
