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
      hash = "sha256-MWdwHeWcMGRRD0H1Alg2JsQ2iEidLCxLCA/hopsBWG4=";
      # hash = lib.fakeHash;
    };

    cargoHash = "sha256-+X7udwcDncOfm18s/4I7khG2xkjZXADd9sWQtY/ce78=";
    # cargoHash = lib.fakeHash;
    strip = true;

    meta = {
      description = "an AIS upload and forwarding tool";
      homepage = "https://github.com/fegies/ais-uploader";
      license = lib.licenses.mit;
    };
  }
)
