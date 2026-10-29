{
  description = "A lightweight, native Linux launcher for RuneLite.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = forAllSystems (system: import nixpkgs { inherit system; });
    in {
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor.${system};
          curl-impersonate-version = "1.5.0";
          curl-impersonate-arch = if system == "x86_64-linux" then "x86_64" else "aarch64";

          curl-impersonate = pkgs.fetchurl {
            url = "https://github.com/lexiforest/curl-impersonate/releases/download/v${curl-impersonate-version}/libcurl-impersonate-v${curl-impersonate-version}.${curl-impersonate-arch}-linux-gnu.tar.gz";
            sha256 = if system == "x86_64-linux" then
                       "f07e25084020c54d6fd5654c8d458e09b3a44c312f88e480c255399f00487b25"
                     else
                       "b4e4f713655616efd2be83153d9057b5961c15e34563dde09a8b6798a8b331e9";
          };

        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "rlshim";
            version = "1.5.0";

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = with pkgs; [
              pkg-config
            ];

            buildInputs = with pkgs; [
              libsecret
              stdenv.cc.cc.lib
            ];

            preBuild = ''
              mkdir -p _bundled
              cp ${curl-impersonate} _bundled/libcurl-impersonate.tar.gz
            '';

            postInstall = ''
              install -Dm644 rlshim.desktop "$out/share/applications/rlshim.desktop"
              install -Dm644 data/icon.png "$out/share/pixmaps/rlshim.png"
              install -d "$out/share/rlshim/data"
              install -m644 data/* "$out/share/rlshim/data/"
            '';
          };
        }
      );
    };
}
