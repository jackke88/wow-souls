{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.wow-souls;
  inherit (lib)
    literalExpression
    mkEnableOption
    mkIf
    mkOption
    types
    ;
in
{
  options.programs.wow-souls = {
    enable = mkEnableOption "WoW Souls, a single-player souls-like prototype in the World of Warcraft 1.12.1 world";

    package = mkOption {
      type = types.package;
      default = pkgs.wow-souls;
      defaultText = literalExpression "pkgs.wow-souls";
      description = "The wow-souls derivation to install. It must come from this flake: the options below are arguments of its wrapper.";
    };

    wowData = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "/home/alice/games/WoW_1.12.1";
      description = ''
        Absolute path to the player's own World of Warcraft 1.12.1 (build 5875)
        client: either the folder holding WoW.exe or its Data folder; the wrapper
        hands the engine the one holding dbc.mpq, model.mpq and terrain.mpq. When
        set, the game starts without asking for it. Kept a string on purpose, so
        the client is never copied into the Nix store.
      '';
    };

    dsrRoot = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "/home/alice/.steam/steam/steamapps/common/Dark Souls Remastered";
      description = ''
        Absolute path to the player's own Dark Souls Remastered install, the folder
        holding DarkSoulsRemastered.exe and chr/c0000.anibnd.dcx. With it set, the
        wrapper builds the combat animations itself the first time the game starts
        (wow-souls-build-animations does the same by hand, and --force again); the
        library lands in the state folder, where the game looks for it. When null,
        the game asks for the folder in a dialog instead.
      '';
    };

    animDir = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = ''
        The built animation library the game reads (WOWSOULS_ANIM_DIR). Its presence
        also tells the game the animations are taken care of, so it neither builds
        them nor asks for Dark Souls: point it at a folder wow-souls-build-animations
        has filled. Default: the state folder's WoWSouls/anim.
      '';
    };

    savesDir = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "/home/alice/.local/share/wow-souls";
      description = "State folder for saves, settings, the remembered client folder, the animation library and its build log (LOCALAPPDATA; the game keeps its own files under its WoWSouls subfolder). A leading ~ is not expanded. Default: $XDG_DATA_HOME/wow-souls.";
    };

    spokenDir = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Root of a Spoken Player voice pack; the string \"0\" turns quest voices off.";
    };

    extraEnv = mkOption {
      type = types.attrsOf types.str;
      default = { };
      example = {
        WOW_AO = "0";
      };
      description = "Extra variables the wrapper exports, each only when the user has not set it.";
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [
      (cfg.package.override {
        inherit (cfg)
          wowData
          dsrRoot
          animDir
          savesDir
          spokenDir
          extraEnv
          ;
      })
    ];

    # A folder the wrapper was not told about is a folder the game asks for, and the
    # asking goes through the freedesktop portal: without one the player gets no
    # dialog and no game. Worth saying at build time rather than at the first start.
    warnings =
      let
        needsDialog = cfg.wowData == null || cfg.dsrRoot == null;
        hasPortal = (config ? xdg.portal.enable) && config.xdg.portal.enable;
      in
      lib.optionals (needsDialog && !hasPortal) [
        ''
          programs.wow-souls: ${if cfg.wowData == null then "wowData" else "dsrRoot"} is unset,
          so the game asks for that folder in a dialog at its first start, but
          xdg.portal.enable is false, so there is no portal to answer it. Set the
          option to the folder, or enable xdg.portal with a portal implementation.
        ''
      ];
  };
}
