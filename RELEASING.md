# Releasing

A release is a git tag, `vX.Y.Z`, and consumers pin it (`tag = "vX.Y.Z"`). The Release workflow tags a commit that is
already on `main` and has passed CI, and writes the GitHub Release notes from the commits since the previous
tag. It commits nothing, so the tag, not the `version` in Cargo.toml, is the release version. Choose patch, minor or
major; the first release takes the version in Cargo.toml without its prerelease suffix (`0.1.0-alpha.1` gives
`v0.1.0`; pass `-f version=0.1.0-alpha.1` to keep it).

```sh
gh workflow run release.yml -R microscaler/BRRTRouter -f bump=patch
```

`-f ref=<sha>` releases an earlier green commit on `main` (it must come after the newest release);
`-f version=X.Y.Z` sets the version outright.
