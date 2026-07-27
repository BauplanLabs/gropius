lint:
    cargo clippy --all-targets --all-features -- -Dwarnings
    cargo clippy --all-targets --features client-ureq -- -Dwarnings
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features client-reqwest

    # Lint CI/CD.
    zizmor . --persona pedantic

[env("INSTA_UPDATE", "no")]
test: lint
    cargo test --all-features
    cargo test --features client-ureq

[env("INSTA_UPDATE", "always")]
[env("TRYBUILD", "overwrite")]
update-snapshots:
    cargo test
