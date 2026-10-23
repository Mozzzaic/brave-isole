# Contributing

This is an early macOS developer preview. Please open a small issue describing the behavior before proposing a large change. Use synthetic pages and accounts in reproductions.

Run the following before submitting changes:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
node --test extension/tests/*.test.mjs
python3 -m unittest discover -s tests -p '*_test.py'
```

Changes to native code, entitlements, signing or IPC require new signed-build isolation checks and real-client launch verification before activation. CI unit tests are not a substitute for those installation checks. Never weaken the gate just to get an installation working.

Keep private data, generated app bundles, client configuration, account screenshots, role secrets and `work/` outside Git. The extension public key is deliberately public and is the only narrowly allowed secret-scanner false positive.

Use [private vulnerability reporting](SECURITY.md) for security findings. Other focused bug reports and documentation improvements are welcome. By contributing, you agree your contribution may be distributed under this repository's MIT license. Dependency licenses remain their own.
