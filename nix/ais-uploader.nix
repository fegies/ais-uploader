{
  fetchFromGitHub,
  rustPlatform,
  pkg-config,
  openssl,
  lib,
}:
rustPlatform.buildRustPackage (
  finalAttrs: {
    pname = "ais_forwarder";
    version = "0.1.1";

    nativeBuildInputs = [pkg-config];
    buildInputs = [openssl];

    src = fetchFromGitHub {
      owner = "orbitsailor";
      repo = "ais-uploader";
      tag = finalAttrs.version;
      hash = "sha256-EwOE//YOBKFH3NC1z3AOEAMGqAPj7cdhQmUB3I6su+Y=";
    };

    cargoHash = "sha256-9L2dzGMp1ttuoAbBDjQPi46i59Q1uV9PTTaqRxsW/AU=";
    strip = true;

    meta = {
      description = "an AIS upload and forwarding tool";
      homepage = "https://github.com/fegies/ais-uploader";
      license = lib.licenses.mit;
    };
  }
)
