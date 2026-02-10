{
  lib,
  fetchPypi,
  buildPythonPackage,

  # build-system
  setuptools,

  # dependencies
  av,
  ctranslate2,
  huggingface-hub,
  onnxruntime,
  tokenizers,
}:

buildPythonPackage rec {
  pname = "faster-whisper";
  version = "1.0.3";
  pyproject = true;

  src = fetchPypi {
    pname = "faster_whisper";
    inherit version;
    hash = lib.fakeHash;
  };

  build-system = [
    setuptools
  ];

  dependencies = [
    av
    ctranslate2
    huggingface-hub
    onnxruntime
    tokenizers
  ];

  # Tests require models
  doCheck = false;

  pythonImportsCheck = [ "faster_whisper" ];

  meta = {
    description = "Faster Whisper transcription with CTranslate2";
    homepage = "https://github.com/guillaumekln/faster-whisper";
    license = lib.licenses.mit;
  };
}
