{
  flake,
  inputs,
  pkgs,
  ...
}:
let
  treefmt-settings = {
    package = pkgs.treefmt;
    projectRootFile = "flake.nix";
    programs = {
      # nix
      deadnix.enable = true;
      nixfmt.enable = true;

      # shell
      shellcheck.enable = true;
      shfmt.enable = true;

      # yaml
      yamlfmt.enable = true;
      yamlfmt.settings.formatter = {
        type = "basic";
        indent = 2;
        retain_line_breaks = true;
      };

      # json
      jsonfmt.enable = true;

      # just
      just.enable = true;

      # python
      ruff-format.enable = true;
      ruff-check.enable = true;

      # markdown
      mdformat.enable = true;

      # toml
      taplo.enable = true;
    };
    settings = {
      # nix
      formatter.deadnix.pipeline = "nix";
      formatter.deadnix.priority = 1;
      formatter.nixfmt.pipeline = "nix";
      formatter.nixfmt.priority = 2;

      # shell
      formatter.shellcheck.pipeline = "shell";
      formatter.shellcheck.includes = [
        "*.sh"
        "*.bash"
        "*.envrc"
        "*.envrc.*"
        "bin/*"
      ];
      formatter.shellcheck.priority = 1;
      formatter.shfmt.pipeline = "shell";
      formatter.shfmt.includes = [
        "*.sh"
        "*.bash"
        "*.envrc"
        "*.envrc.*"
        "bin/*"
      ];
      formatter.shfmt.priority = 2;

      # yaml
      formatter.yamlfmt.pipeline = "yaml";
      formatter.yamlfmt.priority = 1;

      # python
      formatter.ruff-check.pipeline = "python";
      formatter.ruff-check.priority = 1;
      formatter.ruff-format.pipeline = "python";
      formatter.ruff-format.priority = 2;

      # markdown
      formatter.mdformat.pipeline = "markdown";
      formatter.mdformat.priority = 1;

      # toml
      formatter.taplo.pipeline = "toml";
      formatter.taplo.priority = 1;
    };
  };

  formatter = inputs.treefmt-nix.lib.mkWrapper pkgs treefmt-settings;

  check =
    pkgs.runCommand "format-check"
      {
        nativeBuildInputs = [
          formatter
          pkgs.git
        ];

        # only check on Linux
        meta.platforms = pkgs.lib.platforms.linux;
      }
      ''
        export HOME=$NIX_BUILD_TOP/home

        # keep timestamps so that treefmt is able to detect mtime changes
        cp --no-preserve=mode --preserve=timestamps -r ${flake} source
        cd source
        git init --quiet
        git add .
        treefmt --no-cache
        if ! git diff --exit-code; then
          echo "-------------------------------"
          echo "aborting due to above changes ^"
          exit 1
        fi
        touch $out
      '';
in
formatter
// {
  meta = formatter.meta // {
    tests = {
      check = check;
    };
  };
}
