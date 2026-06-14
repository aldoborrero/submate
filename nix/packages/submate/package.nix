{
  lib,
  rustPlatform,
  cmake,
  clang,
  gnumake,
  pkg-config,
  cudaPackages,
  autoAddDriverRunpath,
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
# Builds the `submate` CLI; the `model` feature compiles whisper.cpp via
# whisper-rs (needs cmake + a C/C++ toolchain + libclang for bindgen). A GPU
# backend adds its toolkit + the matching cargo feature.
rustPlatform.buildRustPackage {
  pname = "submate" + lib.optionalString (gpuBackend != null) "-${gpuBackend}";
  version = "0.1.0";

  # The Rust workspace lives at the repo root; select just its files so a build
  # doesn't sweep in nix/, .scratch/, target/, docs/, etc.
  src = lib.fileset.toSource {
    root = ../../..;
    fileset = lib.fileset.unions [
      ../../../Cargo.toml
      ../../../Cargo.lock
      ../../../crates
    ];
  };
  cargoLock.lockFile = ../../../Cargo.lock;

  nativeBuildInputs = [
    rustPlatform.bindgenHook # LIBCLANG_PATH + clang args for whisper-rs bindgen
    cmake # whisper-rs-sys builds whisper.cpp with cmake
    clang
    gnumake
    pkg-config
  ]
  # autoAddDriverRunpath patches the binary's runpath to the host NVIDIA driver
  # (`/run/opengl-driver/lib`) so the real `libcuda` is found at runtime — we
  # link against a stub at build time (see preBuild), not the driver itself.
  ++ lib.optionals isCuda [
    cudaPackages.cuda_nvcc
    autoAddDriverRunpath
  ]
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

  # whisper.cpp's CUDA build links the driver lib `-lcuda`, which exists only at
  # runtime. The toolkit ships a build-time stub at `cuda_cudart/lib/stubs`;
  # point the linker there so it resolves in the sandbox. This adds only a `-L`
  # search path (no runpath), so the stub never leaks into the binary — the real
  # driver is wired in by autoAddDriverRunpath above.
  preBuild = lib.optionalString isCuda ''
    export NIX_LDFLAGS="''${NIX_LDFLAGS:-} -L${cudaPackages.cuda_cudart}/lib/stubs"
  '';

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
