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
    version = "0.1.2";

    nativeBuildInputs = [pkg-config];
    buildInputs = [openssl];

    src = fetchFromGitHub {
      owner = "orbitsailor";
      repo = "ais-uploader";
      tag = finalAttrs.version;
      hash = "sha256-LGfqzldmBzJniSb6paDqoLwFTuO0jQvgc1en12DNBKg=";
      # hash = lib.fakeHash;
    };

    cargoHash = "sha256-LLy0oiKJ5RLe2LPDdfh+90Sl4U89tudkzj4JQNkdJ7g=";
    # cargoHash = lib.fakeHash;
    strip = true;

    meta = {
      description = "an AIS upload and forwarding tool";
      homepage = "https://github.com/orbitsailor/ais-uploader";
      license = lib.licenses.mit;
    };
  }
)
