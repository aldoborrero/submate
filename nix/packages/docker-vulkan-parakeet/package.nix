{
  dockerTools,
  buildEnv,
  submate-vulkan-parakeet,
  ffmpeg,
  cacert,
  busybox,
  curl,
  vulkan-loader,
}:

let
  env = buildEnv {
    name = "submate-vulkan-parakeet-env";
    paths = [
      submate-vulkan-parakeet # native-Rust submate, Vulkan whisper.cpp + Parakeet
      ffmpeg # audio extraction / decode
      cacert # TLS roots for the LLM translation backends
      vulkan-loader # libvulkan.so loader (the ICD + driver come from the host)
      busybox
      curl # healthcheck
    ];
    pathsToLink = [
      "/bin"
      "/etc"
      "/lib"
      "/share"
    ];
  };
in
# Vulkan image carrying BOTH engines: whisper.cpp (default) and transcribe.cpp/
# Parakeet, selectable per request with `submate transcribe --engine parakeet`
# (Parakeet needs its own model via --model / SUBMATE__PARAKEET__MODEL). Same
# runtime GPU wiring as the plain Vulkan image:
#   docker run --device /dev/dri \
#     -v /usr/share/vulkan/icd.d:/usr/share/vulkan/icd.d:ro \
#     -p 9000:9000 submate:vulkan-parakeet
dockerTools.buildLayeredImage {
  name = "submate";
  tag = "vulkan-parakeet";

  contents = [ env ];

  config = {
    Entrypoint = [ "/bin/submate" ];
    Cmd = [ "server" ];
    Env = [
      "PATH=/bin"
      "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "SUBMATE__WHISPER__DEVICE=vulkan"
    ];
    WorkingDir = "/data";
    Volumes = {
      "/data" = { };
      "/root/.cache/huggingface" = { };
    };
    ExposedPorts = {
      "9000/tcp" = { };
    };
    Labels = {
      "org.opencontainers.image.title" = "submate";
      "org.opencontainers.image.description" =
        "Subtitle generation with Whisper + Parakeet, Rust port (Vulkan)";
    };
    Healthcheck = {
      Test = [
        "CMD"
        "/bin/curl"
        "-f"
        "http://localhost:9000/"
      ];
      Interval = 30000000000;
      Timeout = 10000000000;
      Retries = 3;
    };
  };
}
