{
  dockerTools,
  submate,
  ffmpeg,
  cacert,
  busybox,
  curl,
}:

# GPU image - requires nvidia-container-toolkit on host
# Run with: docker run --gpus all ghcr.io/aldoborrero/submate:gpu
dockerTools.buildLayeredImage {
  name = "submate";
  tag = "gpu";

  contents = [
    submate
    ffmpeg
    cacert
    busybox
    curl
  ];

  config = {
    Entrypoint = [ "${submate}/bin/submate" ];
    Cmd = [ "server" ];
    Env = [
      "SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt"
      # Signal that this image supports GPU
      "NVIDIA_VISIBLE_DEVICES=all"
      "NVIDIA_DRIVER_CAPABILITIES=compute,utility"
      # Default to CUDA device
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
      "org.opencontainers.image.description" = "Subtitle generation with Whisper (GPU)";
      "com.nvidia.volumes.needed" = "nvidia_driver";
    };
    Healthcheck = {
      Test = [
        "CMD"
        "curl"
        "-f"
        "http://localhost:9000/"
      ];
      Interval = 30000000000; # 30s in nanoseconds
      Timeout = 10000000000; # 10s in nanoseconds
      Retries = 3;
    };
  };
}
