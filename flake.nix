{
  description = "Aeris - Unbounded Package Management";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell rec {
          buildInputs = with pkgs; [
            libxkbcommon
            libX11
            libxcb
            libXcursor
            libXi
            libXrandr
            lld
            pkg-config
            vulkan-loader
            wayland
          ];

          shellHook = ''
            export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${toString (pkgs.lib.makeLibraryPath buildInputs)}";
          '';
        };
      }
    );
}
