{ pkgs, perSystem }:
perSystem.devshell.mkShell {
  packages = (
    with pkgs;
    [
      ffmpeg # audio extraction / decode
      yt-dlp # fetch test videos from YouTube etc. (uses ffmpeg for muxing)
      just
      perSystem.self.formatter

      # Rust toolchain for the submate workspace (kept in sync within one nixpkgs rev).
      cargo
      rustc
      clippy
      rustfmt
      rust-analyzer
      cargo-audit

      # whisper-rs builds whisper.cpp (cmake + a build tool) and runs bindgen.
      clang
      cmake
      gnumake
      pkg-config
    ]
  );

  env = [
    {
      name = "NIX_PATH";
      value = "nixpkgs=${toString pkgs.path}";
    }
    {
      name = "NIX_DIR";
      eval = "$PRJ_ROOT/nix";
    }
    {
      # bindgen (whisper-rs) needs to locate libclang.
      name = "LIBCLANG_PATH";
      value = "${pkgs.libclang.lib}/lib";
    }
  ];

  commands = [
  ];
}
