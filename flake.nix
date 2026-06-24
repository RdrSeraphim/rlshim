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
          curl-impersonate-version = "1.5.6";
          curl-impersonate-arch = if system == "x86_64-linux" then "x86_64" else "aarch64";
          
          curl-impersonate = pkgs.fetchurl {
            url = "https://github.com/lexiforest/curl-impersonate/releases/download/v${curl-impersonate-version}/libcurl-impersonate-v${curl-impersonate-version}.${curl-impersonate-arch}-linux-gnu.tar.gz";
            sha256 = if system == "x86_64-linux" then
                       "f07e25084020c54d6fd5654c8d458e09b3a44c312f88e480c255399f00487b25"
                     else
                       "b4e4f713655616efd2be83153d9057b5961c15e34563dde09a8b6798a8b331e9";
          };

          json-src = pkgs.fetchFromGitHub {
            owner = "nlohmann";
            repo = "json";
            rev = "v3.12.0";
            hash = "sha256-cECvDOLxgX7Q9R3IE86Hj9JJUxraDQvhoyPDF03B2CY=";
          };

          glfw-src = pkgs.fetchFromGitHub {
            owner = "glfw";
            repo = "glfw";
            rev = "3.3.8";
            hash = "sha256-4+H0IXjAwbL5mAWfsIVhW0BSJhcWjkQx4j2TrzZ3aIo=";
          };

          imgui-src = pkgs.fetchFromGitHub {
            owner = "ocornut";
            repo = "imgui";
            rev = "v1.92.8";
            hash = "sha256-NaSgDE5QEEMrsgYOCAPx5d0XvCIQ9T+ciAKfLFhlmzw=";
          };

        in {
          default = pkgs.stdenv.mkDerivation {
            pname = "rlshim";
            version = "1.1.1";

            src = ./.;

            nativeBuildInputs = with pkgs; [
              cmake
              pkg-config
              clang
            ];

            buildInputs = with pkgs; [
              libsecret
              openssl
              glfw
              libGL
              curl
              libx11
              libxrandr
              libxinerama
              libxcursor
              libxi
            ];

            cmakeFlags = [
              "-DFETCHCONTENT_SOURCE_DIR_JSON=${json-src}"
              "-DFETCHCONTENT_SOURCE_DIR_GLFW=${glfw-src}"
              "-DFETCHCONTENT_SOURCE_DIR_IMGUI=${imgui-src}"
            ];

            preConfigure = ''
              # Pre-populate the curl-impersonate extracted directory so CMake skips the download
              mkdir -p build/_deps/curl-impersonate/extracted
              tar -xzf ${curl-impersonate} -C build/_deps/curl-impersonate/extracted
            '';
          };
        }
      );
    };
}
