lint:
    cargo clippy -- -Dwarnings
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

    # Lint CI/CD.
    # zizmor . --persona pedantic

[env("INSTA_UPDATE", "no")]
test: lint
    cargo test

[env("INSTA_UPDATE", "always")]
update-snapshots:
    cargo test
