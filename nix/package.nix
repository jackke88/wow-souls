# WoW Souls: the game (crates/wow-souls) plus its GPL animation builder
# (crates/dsr-cache, a separate cargo workspace), packaged as one system-wide
# installation.
#
# The game reads a World of Warcraft 1.12.1 client and the player's own Dark Souls
# Remastered from disk; nothing of that is in here. The wrapper points the game at
# the packaged project data (data/, anim/) and gives the Windows-shaped
# %LOCALAPPDATA% state directory a sane Linux home.
{
  lib,
  stdenv,
  rustPlatform,
  pkg-config,
  alsa-lib,
  libglvnd,
  udev,
  vulkan-loader,
  wayland,
  wayland-protocols,
  libxkbcommon,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
  libxcb,
  src,
  version,
  # `release` or `ship`; `ship` is upstream's shipping profile (fat LTO, one
  # codegen unit) and takes noticeably longer to build.
  profile ? "release",
  # Cargo vendor hashes of the two workspaces. Refresh both together with the
  # source: `nix build .#game` and `nix build .#wow-souls-dsr` each report the
  # expected hash when their Cargo.lock no longer matches.
  cargoHash ? "sha256-DWuG3O+8lkKByDMA4bYdqanaUyKF82VBe90auAe6/8U=",
  dsrCargoHash ? "sha256-zMc75DjM43tT4EfgLD6zXOmi+SjS2sO1p8MZDhI1q1Q=",
  # Absolute path to the player's WoW 1.12.1 folder (the one holding WoW.exe and
  # Data, or the Data folder itself). When set, the game starts without asking.
  wowData ? null,
  # Absolute path to a Dark Souls Remastered install, used to build the combat
  # animations once. With it set, `wow-souls` builds them at its own first start;
  # without it the game asks for the folder in a dialog. The game never reads this
  # itself: it is the wrapper's `WOWSOULS_DSR`, passed to the builder as `--dsr`.
  dsrRoot ? null,
  # Absolute path to the folder the built animation library lives in
  # (WOWSOULS_ANIM_DIR). Default: the state dir under LOCALAPPDATA.
  animDir ? null,
  # State folder for saves, settings, the chosen WoW folder and the animation
  # library. Default: $XDG_DATA_HOME/wow-souls.
  savesDir ? null,
  # Absolute path to a Spoken Player voice pack root; "0" turns voices off.
  spokenDir ? null,
  # Extra variables the wrapper exports (only when the user has not set them).
  extraEnv ? { },
}:
let
  inherit (lib)
    concatStringsSep
    escapeShellArg
    makeLibraryPath
    mapAttrsToList
    optional
    ;

  # Linked at build time (alsa, libudev) or dlopened at runtime (X11, Wayland,
  # EGL/Vulkan); the wrapper puts them on LD_LIBRARY_PATH.
  nativeDeps = [
    alsa-lib
    libglvnd
    udev
    vulkan-loader
    wayland
    wayland-protocols
    libx11
    libxcursor
    libxi
    libxrandr
    libxcb
    libxkbcommon
  ];

  runtimeLibs = makeLibraryPath nativeDeps;

  game = rustPlatform.buildRustPackage {
    pname = "wow-souls-game";
    inherit version src cargoHash;

    nativeBuildInputs = [ pkg-config ];
    buildInputs = nativeDeps;

    # The tests read the developer's checkout (anim/source, cache dirs).
    doCheck = false;

    cargoBuildType = profile;
    # The player build: `dev` is the developer seam, carrying engine instruments
    # and a source path (this is what upstream's Play.bat builds).
    cargoBuildFlags = [
      "-p"
      "wow-souls"
      "--no-default-features"
    ];

    meta = {
      description = "Single-player souls-like action prototype in the World of Warcraft 1.12.1 world";
      homepage = "https://github.com/jackke88/wow-souls";
      license = with lib.licenses; [
        mit
        asl20
      ];
      platforms = [ "x86_64-linux" ];
      mainProgram = "wow-souls";
      sourceProvenance = with lib.sourceTypes; [ fromSource ];
    };
  };

  # The GPL-3.0 animation builder, a separate workspace so its GPL code never
  # links into the game. The game looks for it beside its own executable.
  dsr = rustPlatform.buildRustPackage {
    pname = "wow-souls-dsr";
    inherit version src;
    cargoHash = dsrCargoHash;
    # The nested workspace: vendored from its own Cargo.lock, built in its own
    # directory, with the binary in the shared target directory.
    cargoRoot = "crates/dsr-cache";
    buildAndTestSubdir = "crates/dsr-cache";

    nativeBuildInputs = [ pkg-config ];
    buildInputs = nativeDeps;
    doCheck = false;
    cargoBuildType = "release";

    meta = {
      description = "Animation builder for wow-souls: retargets Dark Souls Remastered clips onto WoW skeletons";
      homepage = "https://github.com/jackke88/wow-souls";
      license = with lib.licenses; [ gpl3Plus ];
      platforms = [ "x86_64-linux" ];
      mainProgram = "wow-souls-dsr";
      sourceProvenance = with lib.sourceTypes; [ fromSource ];
    };
  };

  # Lines of the environment file the wrapper sources. Each one keeps a value the
  # user already exported, so the wrapper never gets in the way. `$WS_ROOT` is the
  # package's own prefix, set by the wrapper before it sources this.
  envLines = [
    ''export WOWSOULS_DATA_DIR="''${WOWSOULS_DATA_DIR:-$WS_ROOT/share/wow-souls}"''
  ]
  ++ optional (savesDir != null) ''export LOCALAPPDATA="''${LOCALAPPDATA:-${savesDir}}"''
  ++ [
    ''if [ -z "''${LOCALAPPDATA:-}" ]; then''
    ''LOCALAPPDATA="''${XDG_DATA_HOME:-''${HOME:-/tmp}/.local/share}/wow-souls"''
    "export LOCALAPPDATA"
    "fi"
    ''mkdir -p "''${LOCALAPPDATA}" 2>/dev/null || :''
  ]
  ++ optional (wowData != null) ''export WOW_DATA="''${WOW_DATA:-${wowData}}"''
  ++ optional (dsrRoot != null) ''export WOWSOULS_DSR="''${WOWSOULS_DSR:-${dsrRoot}}"''
  ++ optional (animDir != null) ''export WOWSOULS_ANIM_DIR="''${WOWSOULS_ANIM_DIR:-${animDir}}"''
  ++ optional (spokenDir != null) ''export WOWSOULS_SPOKEN="''${WOWSOULS_SPOKEN:-${spokenDir}}"''
  # `WOW_DATA` is the folder the engine reads its archives from, so it has to be
  # the `Data` folder itself, not the one holding WoW.exe. Players keep giving
  # the latter (it is what the game's own dialog asks for), so accept both here.
  ++ [
    "ws_has_archives() {"
    "for ws_f in dbc model terrain; do"
    ''if [ ! -f "''${1}/''${ws_f}.mpq" ] && [ ! -f "''${1}/''${ws_f}.MPQ" ]; then''
    "return 1"
    "fi"
    "done"
    "}"
    ''if [ -n "''${WOW_DATA:-}" ] && ! ws_has_archives "''${WOW_DATA}"; then''
    ''for ws_c in "''${WOW_DATA}/Data" "''${WOW_DATA}/data"; do''
    ''if ws_has_archives "''${ws_c}"; then''
    ''WOW_DATA="''${ws_c}"''
    "export WOW_DATA"
    "break"
    "fi"
    "done"
    "fi"
  ]
  ++ mapAttrsToList (name: value: ''export ${name}="''${${name}:-${toString value}}"'') extraEnv;

in
stdenv.mkDerivation {
  pname = "wow-souls";
  inherit version;

  phases = [ "installPhase" ];

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin $out/libexec/wow-souls $out/share/wow-souls $out/share/applications

    # The real binaries live in libexec: the wrapper is what the user starts, and
    # the game finds its builder beside the executable it was really started from.
    install -m755 ${game}/bin/wow-souls $out/libexec/wow-souls/wow-souls
    install -m755 ${dsr}/bin/wow-souls-dsr $out/libexec/wow-souls/wow-souls-dsr
    ln -s $out/libexec/wow-souls/wow-souls-dsr $out/bin/wow-souls-dsr

    cp -r ${src}/data $out/share/wow-souls/data
    cp -r ${src}/anim $out/share/wow-souls/anim
    install -m644 ${src}/README.md $out/share/wow-souls/README.md

    printf '%s\n' ${concatStringsSep " " (map escapeShellArg envLines)} \
      > $out/libexec/wow-souls/env

    # The combat animations, built once into the state folder, where the game looks
    # for them itself. `setup` wants the DSR folder, the WoW `Data` folder, the
    # packaged project (its anim/provenance hashes every clip it accepts) and a
    # writable output; only the last one is not in the store.
    cat > $out/libexec/wow-souls/setup-animations <<'WS_SETUP'
    #!/bin/sh
    # Generated by the wow-souls flake: builds the animation library, once. Exits
    # non-zero when it is not there, so callers can fall back to asking the player.
    # --force builds again, --stdout keeps the builder's output on the terminal.
      WS_ROOT=@WS_PREFIX@
    . "$WS_ROOT/libexec/wow-souls/env"
    WS_FORCE=
    WS_TO_STDOUT=
    for ws_a in "$@"; do
      case "$ws_a" in
        --force) WS_FORCE=1 ;;
        --stdout) WS_TO_STDOUT=1 ;;
        *) echo "wow-souls: unknown option $ws_a" >&2; exit 2 ;;
      esac
    done
    WS_ANIM="''${WOWSOULS_ANIM_DIR:-$LOCALAPPDATA/WoWSouls/anim}"
    if [ -z "$WS_FORCE" ] && [ -f "$WS_ANIM/built.txt" ]; then
      exit 0
    fi
    if [ -z "''${WOWSOULS_DSR:-}" ]; then
      echo "wow-souls: no Dark Souls Remastered folder: set WOWSOULS_DSR, or" >&2
      echo "wow-souls: programs.wow-souls.dsrRoot in the NixOS configuration" >&2
      exit 1
    fi
    if [ -z "''${WOW_DATA:-}" ]; then
      echo "wow-souls: no WoW 1.12.1 Data folder: set WOW_DATA, or" >&2
      echo "wow-souls: programs.wow-souls.wowData in the NixOS configuration" >&2
      exit 1
    fi
    mkdir -p "$LOCALAPPDATA/WoWSouls" 2>/dev/null || :
    if [ -n "$WS_TO_STDOUT" ]; then
      "$WS_ROOT/libexec/wow-souls/wow-souls-dsr" setup \
        --dsr "$WOWSOULS_DSR" --wow "$WOW_DATA" \
        --project "$WOWSOULS_DATA_DIR" --out "$WS_ANIM"
    else
      "$WS_ROOT/libexec/wow-souls/wow-souls-dsr" setup \
        --dsr "$WOWSOULS_DSR" --wow "$WOW_DATA" \
        --project "$WOWSOULS_DATA_DIR" --out "$WS_ANIM" \
        >>"$LOCALAPPDATA/WoWSouls/anim-setup.log" 2>&1
    fi
    WS_SETUP
    chmod 755 $out/libexec/wow-souls/setup-animations

    cat > $out/bin/wow-souls-build-animations <<'WS_BUILD'
    #!/bin/sh
    # Generated by the wow-souls flake: (re)builds the combat animations from the
    # player's own Dark Souls Remastered, showing what the builder says. Safe to
    # run again; the game runs the same thing quietly when it starts without them.
      WS_ROOT=@WS_PREFIX@
    exec "$WS_ROOT/libexec/wow-souls/setup-animations" --stdout "$@"
    WS_BUILD
    chmod 755 $out/bin/wow-souls-build-animations

    cat > $out/bin/wow-souls <<'WS_BIN'
    #!/bin/sh
    # Generated by the wow-souls flake: sets up the runtime environment, then execs
    # the game. Anything the user already exported wins.
      WS_ROOT=@WS_PREFIX@
    . "$WS_ROOT/libexec/wow-souls/env"
    export LD_LIBRARY_PATH="${runtimeLibs}''${LD_LIBRARY_PATH:+:''${LD_LIBRARY_PATH}}"
    # With the flake knowing where Dark Souls is, the animations are built here
    # rather than asked for in a dialog; the game then finds `built.txt` and starts
    # straight away. A failure stays in the state folder's anim-setup.log and the
    # game falls back to asking the player for the folder itself.
    if [ -n "''${WOWSOULS_DSR:-}" ]; then
      "$WS_ROOT/libexec/wow-souls/setup-animations" >/dev/null 2>&1 || :
    fi
    # Some of the game's own check dumps land in `cache/...` under the working
    # directory: keep them in the state folder, not wherever a shortcut started.
    cd "$LOCALAPPDATA" 2>/dev/null || :
    exec "$WS_ROOT/libexec/wow-souls/wow-souls" "$@"
    WS_BIN
    chmod 755 $out/bin/wow-souls
      # The prefix is baked in instead of taken from the script's own path: installed
      # through the system profile, these wrappers sit in /run/current-system/sw/bin, and
      # that directory has no libexec next to them, so finding the package relative to
      # argv[0] misses it.
      for ws_f in $out/libexec/wow-souls/setup-animations \
        $out/bin/wow-souls-build-animations $out/bin/wow-souls; do
        sed -i "s|@WS_PREFIX@|$out|g" "$ws_f"
      done

    cat > $out/share/applications/wow-souls.desktop <<'WS_DESKTOP'
    [Desktop Entry]
    Type=Application
    Name=WoW Souls
    Comment=Souls-like prototype in the World of Warcraft 1.12.1 world
    Exec=wow-souls
    Terminal=false
    Categories=Game;
    Keywords=wow;dark-souls;bevy;
    WS_DESKTOP

    runHook postInstall
  '';

  passthru = {
    inherit
      game
      dsr
      nativeDeps
      runtimeLibs
      src
      ;
  };

  meta = game.meta // {
    mainProgram = "wow-souls";
    longDescription = ''
      WoW Souls packaged for NixOS: the Bevy game binary, the GPL Dark Souls
      animation builder it runs as a separate program, and the packaged project
      data (world/quest exports, animation tables, voices).

      The game needs the player's own World of Warcraft 1.12.1 client and their own
      Dark Souls Remastered; neither is downloadable or redistributable, so both are
      given to it at run time (WOW_DATA, or the folder dialog the first time).
    '';
  };
}
