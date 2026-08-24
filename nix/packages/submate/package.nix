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
  spirv-headers,
  # GPU backend: null (CPU), "cuda", or "vulkan". The variant packages set it;
  # it selects the matching cargo feature + adds that backend's build inputs.
  gpuBackend ? null,
  # Also build the transcribe.cpp/Parakeet engine (`--engine parakeet`). When set
  # with a GPU backend, both whisper and Parakeet get that backend.
  parakeet ? false,
}:

let
  isCuda = gpuBackend == "cuda";
  isVulkan = gpuBackend == "vulkan";
  # Cargo feature: the GPU backend (or bare `model`), optionally the Parakeet
  # variant that also forwards the backend to submate-parakeet.
  baseFeature = if gpuBackend == null then "model" else gpuBackend;
  feature =
    if parakeet then
      (if gpuBackend == null then "parakeet" else "parakeet-${gpuBackend}")
    else
      baseFeature;
in
# Builds the `submate` CLI; the `model` feature compiles whisper.cpp via
# whisper-rs (needs cmake + a C/C++ toolchain + libclang for bindgen). A GPU
# backend adds its toolkit + the matching cargo feature.
rustPlatform.buildRustPackage {
  pname =
    "submate"
    + lib.optionalString (gpuBackend != null) "-${gpuBackend}"
    + lib.optionalString parakeet "-parakeet";
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
  # transcribe-cpp (the parakeet backend dep) is a git dependency, so importCargoLock
  # needs its vendored hash — required whether or not the `parakeet` feature is on
  # in this build (the lock entry exists regardless).
  cargoLock.outputHashes = {
    "transcribe-cpp-0.2.1" = "sha256-IHlLk8MzEHnzfk5dvslHXBzpyAeNQyQOuztQLLzousY=";
  };

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
    ]
    # transcribe.cpp's ggml-vulkan additionally does `find_package(SPIRV-Headers
    # CONFIG REQUIRED)`, which whisper.cpp's does not — needed only for a Vulkan
    # Parakeet build, but harmless to include for every Vulkan build.
    ++ lib.optionals isVulkan [ spirv-headers ];

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
