{
  lib,
  fetchPypi,
  buildPythonPackage,

  # build-system
  setuptools,

  # dependencies
  numpy,
  torch,
  torchaudio,
  tqdm,
  more-itertools,
  openai-whisper,
}:

buildPythonPackage rec {
  pname = "stable-ts";
  version = "2.17.5";
  pyproject = true;

  src = fetchPypi {
    inherit pname version;
    hash = "sha256-a0Gvl1O/hngkjHDwMNKBuXJSnVL5tCzUwAFoJt83nok=";
  };

  build-system = [
    setuptools
  ];

  dependencies = [
    numpy
    torch
    torchaudio
    tqdm
    more-itertools
    openai-whisper
  ];

  # Override faster-whisper to use compatible version
  # Note: Using nixpkgs faster-whisper 1.2.1 which may have API changes
  # If issues persist, we can create a custom faster-whisper 1.0.3 package
  propagatedBuildInputs = dependencies;

  # Patch for compatibility with newer faster-whisper (1.2.x)
  # Newer faster-whisper returns dicts instead of namedtuples
  postPatch = ''
    substituteInPlace stable_whisper/whisper_word_level/faster_whisper.py \
      --replace-fail "segment['words'] = [w._asdict() for w in words]" \
                      "segment['words'] = [w._asdict() if hasattr(w, '_asdict') else w for w in words]"
  '';

  # Disable dependency version checks (nixpkgs has newer openai-whisper)
  pythonRemoveDeps = [ "openai-whisper" ];

  # Tests require network access and models
  doCheck = false;

  pythonImportsCheck = [ "stable_whisper" ];

  meta = {
    description = "Stabilized implementation of Whisper";
    homepage = "https://github.com/jianfch/stable-ts";
    license = lib.licenses.mit;
  };
}
