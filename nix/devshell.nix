{ pkgs, perSystem }:
let
  pythonEnv = pkgs.python313.withPackages (
    ps: with ps; [
      anthropic
      click
      fastapi
      faster-whisper
      ffmpeg-python
      google-generativeai
      httpx
      huey
      mypy
      openai
      perSystem.self.stable-ts
      pydantic
      pydantic-settings
      pytest
      pytest-asyncio
      pytest-cov
      pytest-mock
      python-daemon
      python-multipart
      requests
      rich
      sqlalchemy
      srt
      pysubs2
      uvicorn
    ]
  );
in
perSystem.devshell.mkShell {
  packages = (
    with pkgs;
    [
      pythonEnv
      ffmpeg
      ruff
      uv
      just
      perSystem.self.formatter
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
  ];

  commands = [
  ];
}
