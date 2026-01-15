#!/bin/bash

set -e

APP_NAME="rowheel"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor"

echo "Uninstalling RoWheel..."

# Remove binary
if [[ -f "$INSTALL_DIR/$APP_NAME" ]]; then
    echo "Removing binary from $INSTALL_DIR/$APP_NAME..."
    rm "$INSTALL_DIR/$APP_NAME"
else
    echo "Binary not found at $INSTALL_DIR/$APP_NAME (already removed?)"
fi

# Remove desktop entry
if [[ -f "$DESKTOP_DIR/$APP_NAME.desktop" ]]; then
    echo "Removing desktop entry from $DESKTOP_DIR/$APP_NAME.desktop..."
    rm "$DESKTOP_DIR/$APP_NAME.desktop"
else
    echo "Desktop entry not found at $DESKTOP_DIR/$APP_NAME.desktop (already removed?)"
fi

# Remove icons
echo "Removing icons..."
rm -f "$ICON_DIR/64x64/apps/$APP_NAME.png"
rm -f "$ICON_DIR/256x256/apps/$APP_NAME.png"

# Update desktop database if available
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi

# Update icon cache if available
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache "$ICON_DIR" 2>/dev/null || true
fi

echo ""
echo "Uninstallation complete!"
