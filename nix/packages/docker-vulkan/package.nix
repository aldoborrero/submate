{
  dockerTools,
  buildEnv,
  submate-vulkan,
  ffmpeg,
  cacert,
  busybox,
  curl,
  vulkan-loader,
}:

let
  env = buildEnv {
    name = "submate-vulkan-env";
    paths = [
      submate-vulkan # the native-Rust submate binary, Vulkan-accelerated
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
# Vulkan image of the Rust port — cross-vendor GPUs (incl. Intel/AMD iGPU) where
# CUDA isn't available. Unlike the CUDA image (which relies on the
# nvidia-container-toolkit to inject the driver), Vulkan needs the host's GPU
# device and ICD/driver passed in at runtime, e.g.:
#   docker run --device /dev/dri \
#     -v /usr/share/vulkan/icd.d:/usr/share/vulkan/icd.d:ro \
#     -p 9000:9000 submate:vulkan
dockerTools.buildLayeredImage {
  name = "submate";
  tag = "vulkan";

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
      "org.opencontainers.image.description" = "Subtitle generation with Whisper, Rust port (Vulkan)";
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
