{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.mousekey-studio;
in
{
  options.programs.mousekey-studio = {
    enable = lib.mkEnableOption "Mouse & Key Studio, a local-first Logitech device manager";

    package = lib.mkPackageOption pkgs "mousekey-studio" { };

    launchAtLogin = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to start the Mouse & Key Studio agent with graphical sessions.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    services.udev.packages = [ cfg.package ];

    systemd.user.services.mks-agent = {
      description = "Mouse & Key Studio background agent";
      wantedBy = lib.optionals cfg.launchAtLogin [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      partOf = lib.optionals cfg.launchAtLogin [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = lib.getExe' cfg.package "mks-agent";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
