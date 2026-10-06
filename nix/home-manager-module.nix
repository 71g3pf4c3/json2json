# home-manager module for json2json.
#
# Usage:
#   inputs.json2json.url = "github:71g3pf4c3/json2json";
#   ...
#   imports = [ inputs.json2json.homeManagerModules.default ];
#   programs.json2json.enable = true;
#
# Symmetric to the NixOS module: same options, per-user install via
# `home.packages`.
self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.json2json;
in
{
  options.programs.json2json = {
    enable = lib.mkEnableOption "json2json, the lossless JSON to JSON transformer";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      defaultText = "json2json flake package for this system";
      description = "The json2json package to install.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];
  };
}
