{
  dockerTools,
  submate,
  ffmpeg,
  cacert,
  busybox,
  curl,
}:

dockerTools.buildLayeredImage {
  name = "submate";
  tag = "cpu";

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
