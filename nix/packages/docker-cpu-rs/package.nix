{
  dockerTools,
  buildEnv,
  submate-rs,
  ffmpeg,
  cacert,
  busybox,
  curl,
}:

let
  env = buildEnv {
    name = "submate-rs-env";
    paths = [
      submate-rs # the native-Rust submate binary
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
# CPU image of the Rust port. Mirrors the Python `docker-cpu`, but built from
# `submate-rs`. `docker run -p 9000:9000 submate:cpu-rs`.
dockerTools.buildLayeredImage {
  name = "submate";
  tag = "cpu-rs";

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
      "org.opencontainers.image.description" = "Subtitle generation with Whisper, Rust port (CPU)";
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
