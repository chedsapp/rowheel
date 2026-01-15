# RoWheel

A crossplatform (Linux + Windows) Rust application for racing wheel + force feedback support in Roblox

After calibrating your wheel, Rowheel emulates a gamepad recognizeable by Roblox and mirrors your inputs. Haptic effects uploaded to the controller by Roblox are then processed and sent to your racing wheel's force feedback enabled axis.

This should theoretically work with any wheel.

<img width="503" height="442" alt="image" src="https://github.com/user-attachments/assets/b2a4a2e4-8b18-4a5f-a549-1f600b370448" /> <img width="490" height="442" alt="image" src="https://github.com/user-attachments/assets/d5cd6f5c-4c97-45c0-8bfa-31869f3c93a2" />

## Building

### Prerequisites

Both platforms require the [Rust toolchain](https://rustup.rs/).

### Linux

Install the required development libraries:

```bash
# Debian/Ubuntu
sudo apt install libudev-dev

# Fedora
sudo dnf install systemd-devel

# Arch Linux
sudo pacman -S systemd-libs
```

Build and run:

```bash
cargo build --release
./target/release/rowheel
```

### Windows

1. Install the [ViGEm Bus Driver](https://github.com/nefarius/ViGEmBus/releases) (required for virtual gamepad emulation)

2. Build and run:

```powershell
cargo build --release
.\target\release\rowheel.exe
```

## Notice
* Due to the somewhat limited capabilities of gamepads, H-shifter support isn't implemented and probably won't be until I can figure out a solid non-hacky approach.
* For Linux users, Roblox controller haptic support has been historically hit-or-miss. Driving should work just fine, but as of **1/14/26**, rumble updates don't seem to upload to the connected gamepad correctly - so force feedback effects currently aren't felt. *(This problem only affects Sober. When tested in Roblox Studio via Wine/Vinegar it works just fine.)*
