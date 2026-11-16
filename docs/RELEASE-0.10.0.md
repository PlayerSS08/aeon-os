# Aeon 0.10.0: real-hardware networking, input and packages

## What is new

| Area | What | Evidence |
|---|---|---|
| Network core | TCP state machine (sequence numbers, windows, retransmission, FIN/RST), DNS resolver, HTTP/1.1 client (Content-Length, chunked, redirects, limits) replace a one-connection toy that ignored the response body | 24 host tests; QEMU: DHCP, 300 KB download with matching SHA-256, DNS and a page from the public internet |
| Bugs fixed | Ethernet padding was read as TCP data; the gateway's MAC was hard-coded to QEMU's after DHCP (wrong on every real router); the NIC picker bound any Intel network device to the e1000 driver | code review + tests |
| Wired NIC | Realtek RTL8168 driver (the OMEN's port) | descriptor/register logic host-tested; **hardware untested** |
| Wi-Fi stack | WPA2-PSK: PBKDF2, 4-way handshake, AES-CCMP, 802.11 frames, station state machine | RFC vectors, Python cross-check, simulated AP. **No chip driver** |
| Input | USB boot-protocol mouse through xHCI, on-screen pointer, wheel scrolls the log, `mouse` command | QEMU boot test incl. screenshot pixel check |
| Packages | `apt`: Ed25519-signed index with rollback protection, SHA-256 per package, dependencies, safe tar reader, rollback on failure, USB persistence, offline `apt restore`. The old simulated apt (fake packages, fake "Fetched" output) is gone | 20 host tests + 18 QEMU checks including tampered index/package |
| Crypto | SHA-1, SHA-512, AES-128/CCM/key-wrap, HMAC, PBKDF2, Ed25519 verify | published test vectors |
| Randomness | network ids and sequence numbers from an RDRAND-keyed hash generator (gated on CPUID) | unit tests |
| Shell | `curl -o`, `sha256sum`, `mouse`, honest `wifi`/`connect wifi`, `apt` | QEMU tests |

## Using it on the OMEN

1. Format a USB stick FAT32 with the volume label **AEON**. Copy the contents of `usb/` to it (it holds `EFI/BOOT/BOOTX64.EFI`).
2. Boot it in UEFI mode (F9 boot menu). Secure Boot is off on your laptop, so signing does not matter.
3. Plug in an Ethernet cable and type `dhcp`. If it says there is no link or no device, send a photo of the boot log (`log`): the RTL8168 line prints the chip revision.
4. A USB mouse shows a pointer; the touchpad does not work yet.
5. On a PC on the same network run `python tools/aeon_repo.py serve repo --port 8080`, then on Aeon: `apt source http://<that PC>:8080/`, `apt update`, `apt install greeter`.
6. Files you write under `/usb/` stay on the stick (`sync` first). Packages are copied to `/usb/aeonpkg/`.

## Verified, not verified, not done

- **Verified in QEMU/host:** everything in the table except the lines marked otherwise.
- **Not verified on real hardware:** the RTL8168 driver, xHCI with your controller, mouse on real USB devices, FAT32 writes on your stick model, anything involving your screen mode.
- **Not done:** Wi-Fi chip driver, I2C touchpad, NVMe, TLS/HTTPS, per-process address spaces, `execve`. See `docs/OS-LANDSCAPE.md`.
- **Known limits:** image is 572 KB (the test gate was 512 KiB and was already failing at 554 KB before this work; it is now 640 KiB). The RAM filesystem is small (64 entries, 1 MiB). Aeon's USB stick `aeon.cfg` sets `mode=1024x768`; delete that line to use the screen's native mode.

## Repository tool

`tools/aeon_repo.py keygen | build | pubkey | serve`. The signing key is created in `.secrets/` (git-ignored). Back it up:
a new key means every Aeon build must be rebuilt with the new public key.
