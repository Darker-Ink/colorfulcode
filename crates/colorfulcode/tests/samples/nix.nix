{ pkgs ? import <nixpkgs> { }, lib ? pkgs.lib, enableTests ? true }:

/* A small Rust package with a development shell. */
let
  version = "0.4.1";
  src = lib.cleanSource ./.;
  # Extra tools for the shell
  tools = with pkgs; [ cargo rustc rust-analyzer ];
in
rec {
  package = pkgs.rustPlatform.buildRustPackage {
    pname = "colorful";
    inherit version src;
    cargoLock.lockFile = ./Cargo.lock;
    doCheck = enableTests;
    buildInputs = lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];

    meta = {
      description = "Fast syntax highlighting, version ${version}";
      license = lib.licenses.mit;
      platforms = lib.platforms.all;
    };
  };

  shell = pkgs.mkShell {
    packages = tools ++ [ pkgs.just ];
    RUST_LOG = "debug";
    shellHook = ''
      echo "entering dev shell for ${package.pname}"
      export PATH=$PWD/target/debug:$PATH
    '';
  };

  double = x: x * 2;
  greeting = if builtins.length tools > 2 then "many tools" else "few tools";
  config = builtins.fromJSON (builtins.readFile ../config.json);
  home = ~/projects;
}
