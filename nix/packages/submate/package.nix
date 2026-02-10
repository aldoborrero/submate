{
  lib,
  buildPythonApplication,
  pythonOlder,

  # build-system
  setuptools,

  # dependencies
  numpy,
  fastapi,
  requests,
  uvicorn,
  python-multipart,
  ffmpeg-python,
  watchdog,
  huey,
  pydantic,
  pydantic-settings,
  python-daemon,
  srt,
  pysubs2,
  click,
  rich,
  faster-whisper,
  stable-ts,

  # LLM translation backends
  openai,
  anthropic,
  google-generativeai,
}:

buildPythonApplication rec {
  pname = "submate";
  version = "1.0.0";
  pyproject = true;

  disabled = pythonOlder "3.13";

  src = ../../..;

  build-system = [
    setuptools
  ];

  dependencies = [
    numpy
    fastapi
    requests
    uvicorn
    python-multipart
    ffmpeg-python
    watchdog
    huey
    pydantic
    pydantic-settings
    python-daemon
    srt
    pysubs2
    click
    rich
    faster-whisper
    stable-ts

    # LLM translation backends
    openai
    anthropic
    google-generativeai
  ];

  # Tests require models and network
  doCheck = false;

  pythonImportsCheck = [ "submate" ];

  meta = {
    description = "Subtitle generation tool using Whisper";
    homepage = "https://github.com/aldoborrero/submate";
    license = lib.licenses.mit;
    mainProgram = "submate";
  };
}
