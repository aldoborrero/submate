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
dockerTools.buildLayeredImage {
  name = "submate";
  tag = "cpu";

  contents = [ env ];

  config = {
    Entrypoint = [ "/bin/submate" ];
    Cmd = [ "server" ];
    Env = [
      "PATH=/bin"
      "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "SUBMATE__WHISPER__DEVICE=cpu"
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
      "org.opencontainers.image.description" = "Subtitle generation with Whisper (CPU)";
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
