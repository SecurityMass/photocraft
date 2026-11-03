#!/usr/bin/env bash
# Give an installed copy of the app its own name and icon, on the Mac that will run it:
#
#   bash personalize.sh <name> <image> [path of the .app]
#
# The image is cropped to a centered square. The default .app is /Applications/teste.app. The
# bundle is signed again ad hoc, because changing it breaks its signature. The quarantine
# attribute stays: the first launch is still authorized by hand, in System Settings.
set -euo pipefail

usage() {
  echo "uso: bash personalize.sh <nome> <imagem> [caminho do .app]" >&2
  exit 2
}

[[ $# -ge 2 && $# -le 3 ]] || usage
NAME="$1"
IMAGE="$2"
APP="${3:-/Applications/teste.app}"
APP="${APP%/}"

case "$NAME" in
  "" | */* | .*) echo "erro: nome inválido: '$NAME' (não use / nem comece com ponto)" >&2; exit 2 ;;
esac
[[ -f "$IMAGE" ]] || { echo "erro: imagem não encontrada: $IMAGE" >&2; exit 1; }
PLIST="$APP/Contents/Info.plist"
[[ -f "$PLIST" ]] || { echo "erro: app não encontrado: $APP" >&2; exit 1; }

NEW_APP="$(dirname "$APP")/$NAME.app"
if [[ "$NEW_APP" != "$APP" && -e "$NEW_APP" ]]; then
  echo "erro: já existe $NEW_APP; apague ou renomeie antes de continuar" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# ---- icon: a centered square, at every size an .icns holds ---------------------------------------
sips -s format png "$IMAGE" --out "$WORK/source.png" >/dev/null
WIDTH="$(sips -g pixelWidth "$WORK/source.png" | awk '/pixelWidth/ {print $2}')"
HEIGHT="$(sips -g pixelHeight "$WORK/source.png" | awk '/pixelHeight/ {print $2}')"
SIDE=$((WIDTH < HEIGHT ? WIDTH : HEIGHT))
sips -c "$SIDE" "$SIDE" "$WORK/source.png" --out "$WORK/square.png" >/dev/null
ICONSET="$WORK/icon.iconset"
mkdir "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$WORK/square.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$WORK/square.png" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$WORK/icon.icns"
ICON_FILE="$(plutil -extract CFBundleIconFile raw -o - "$PLIST")"
cp "$WORK/icon.icns" "$APP/Contents/Resources/${ICON_FILE%.icns}.icns"

# ---- Info.plist, kept as XML so the app can read its name back ------------------------------------
OLD_ID="$(plutil -extract CFBundleIdentifier raw -o - "$PLIST")"
SLUG="$(printf '%s' "$NAME" | tr '[:upper:]' '[:lower:]' | LC_ALL=C sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')"
[[ -n "$SLUG" ]] || SLUG=app
plutil -replace CFBundleName -string "$NAME" "$PLIST"
plutil -replace CFBundleDisplayName -string "$NAME" "$PLIST"
plutil -replace CFBundleIdentifier -string "${OLD_ID%.*}.$SLUG" "$PLIST"
plutil -convert xml1 "$PLIST"

# ---- bundle: its new name, then a signature that covers everything above ---------------------------
if [[ "$NEW_APP" != "$APP" ]]; then
  mv "$APP" "$NEW_APP"
fi
codesign --force --deep -s - "$NEW_APP"
codesign --verify --deep --strict "$NEW_APP"
# Finder and the Dock read the icon again when the bundle's date changes.
touch "$NEW_APP"

echo "Pronto: $NEW_APP"
