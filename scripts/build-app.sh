#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
"$HOME/.cargo/bin/cargo" build --release --bin brave-isole-probe
app=work/BraveIsoleProbe.app
mkdir -p "$app/Contents/MacOS"
cp packaging/Info.plist "$app/Contents/Info.plist"
cp target/release/brave-isole-probe "$app/Contents/MacOS/brave-isole-probe"
codesign --force --sign - --identifier dev.david.brave-isole.probe --options runtime --entitlements packaging/app.entitlements "$app"
codesign --verify --strict --verbose=2 "$app"
