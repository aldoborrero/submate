{
  dockerTools,
  buildEnv,
  submate-cuda,
  ffmpeg,
  cacert,
  busybox,
  curl,
}:

let
  env = buildEnv {
    name = "submate-cuda-env";
    paths = [
      submate-cuda # the native-Rust submate binary, CUDA-accelerated
      ffmpeg # audio extraction / decode
      cacert # TLS roots for the LLM translation backends
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
# GPU image of the Rust port. Requires nvidia-container-toolkit on the host.
# Run with: docker run --gpus all submate:gpu
dockerTools.buildLayeredImage {
  name = "submate";
  tag = "gpu";

  contents = [ env ];

  config = {
    Entrypoint = [ "/bin/submate" ];
    Cmd = [ "server" ];
    Env = [
      "PATH=/bin"
      "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "NVIDIA_VISIBLE_DEVICES=all"
      "NVIDIA_DRIVER_CAPABILITIES=compute,utility"
      "SUBMATE__WHISPER__DEVICE=cuda"
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
      "org.opencontainers.image.description" = "Subtitle generation with Whisper, Rust port (GPU)";
      "com.nvidia.volumes.needed" = "nvidia_driver";
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
