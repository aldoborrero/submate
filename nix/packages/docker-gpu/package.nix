{
  lib,
  dockerTools,
  buildEnv,
  submate,
  ffmpeg,
  cacert,
  busybox,
  curl,
}:

let
  # Create a merged environment with all binaries in /bin
  env = buildEnv {
    name = "submate-env";
    paths = [
      submate
      ffmpeg
      cacert
      busybox
      curl
    ];
    pathsToLink = [ "/bin" "/etc" "/lib" "/share" ];
  };
in
# GPU image - requires nvidia-container-toolkit on host
# Run with: docker run --gpus all ghcr.io/aldoborrero/submate:gpu
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
        "/bin/curl"
        "-f"
        "http://localhost:9000/"
      ];
      Interval = 30000000000; # 30s in nanoseconds
      Timeout = 10000000000; # 10s in nanoseconds
      Retries = 3;
    };
  };
}
