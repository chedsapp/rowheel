#!/bin/bash
# Thank you claude code for being so awesome and writing random shell scripts for me I love you

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/target/release"
APP_NAME="rowheel"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor"
ICO_SRC="$SCRIPT_DIR/ico"

# Find the most recent executable in target/release
if [[ ! -d "$RELEASE_DIR" ]]; then
    echo "Error: Release directory not found at $RELEASE_DIR"
    echo "Please run 'cargo build --release' first"
    exit 1
fi

# Find the most recently modified executable (excluding .d files and directories)
BINARY=$(find "$RELEASE_DIR" -maxdepth 1 -type f -executable ! -name "*.d" -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)

if [[ -z "$BINARY" ]]; then
    echo "Error: No executable found in $RELEASE_DIR"
    echo "Please run 'cargo build --release' first"
    exit 1
fi

BINARY_NAME=$(basename "$BINARY")
echo "Found binary: $BINARY_NAME"

# Create install directories if they don't exist
mkdir -p "$INSTALL_DIR"
mkdir -p "$DESKTOP_DIR"
mkdir -p "$ICON_DIR/64x64/apps"
mkdir -p "$ICON_DIR/256x256/apps"

# Copy binary to install location
echo "Installing $BINARY_NAME to $INSTALL_DIR..."
cp "$BINARY" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

# Install icons
echo "Installing icons..."
cp "$ICO_SRC/rowheel-64.png" "$ICON_DIR/64x64/apps/$APP_NAME.png"
cp "$ICO_SRC/rowheel-256.png" "$ICON_DIR/256x256/apps/$APP_NAME.png"

# Create .desktop file
echo "Creating desktop entry..."
cat > "$DESKTOP_DIR/$APP_NAME.desktop" << EOF
[Desktop Entry]
Type=Application
Name=RoWheel
Comment=Emulate gamepads through DirectInput devices like steering wheels
Exec=$INSTALL_DIR/$BINARY_NAME
Icon=$APP_NAME
Terminal=false
Categories=Game;Utility;
Keywords=controller;gamepad;steering;wheel;xbox;
EOF

# Update desktop database if available
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi

# Update icon cache if available
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache "$ICON_DIR" 2>/dev/null || true
fi

echo ""
echo "Installation complete!"
echo "Binary installed to: $INSTALL_DIR/$BINARY_NAME"
echo "Desktop entry created at: $DESKTOP_DIR/$APP_NAME.desktop"
echo ""
echo "Make sure $INSTALL_DIR is in your PATH."
echo "You can now launch RoWheel from your application menu or by running '$BINARY_NAME'"
