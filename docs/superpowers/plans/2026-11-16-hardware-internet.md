# Plan: real-hardware internet, input and packages (v0.10.0)

Target machine: HP OMEN 16-am0xxx. Wi-Fi Realtek RTL8852BE (10EC:B520), wired Realtek RTL8168 (10EC:8168),
touchpad ELAN07F0 over I2C HID, Intel xHCI 8086:7A60, Samsung NVMe (never written: Windows/BitLocker).
Only the AEON-labelled USB volume is writable.

Rule: nothing is called working unless a host test or a VM boot proves it. Code that can only be proven on the
laptop is labelled "untested on hardware" in code docs, the shell and the release notes.

1. Wi-Fi security core (host-tested against independent Python references): sha1/hmac/pbkdf2 (`sha1`),
   AES-128 + key unwrap + CCM (`aes`), WPA2-PSK PMK/PTK/MIC and the EAPOL 4-way handshake (`wpa`).
2. 802.11 layer (`ieee80211`): beacon/probe parsing, auth/assoc builders, CCMP data frames <-> Ethernet.
   `wifi`: `WifiRadio` trait and the connect state machine, tested against a simulated AP.
3. Wired: RTL8168 driver (`rtl8168`), registered as a `NetDev`. Untested on hardware.
4. Input: USB HID mouse through xHCI plus an on-screen pointer; `mouse` shell command.
5. Packages: make `apt` honest (real index and package download over HTTP, SHA-256 checked, Aeon package
   format), no fake "Fetched" output.
6. Boot animation: new design, integer-only maths in `anim`.
7. Docs: OS comparison, README, cmd.md, VERSIONS; bump to 0.10.0; commit on the feature branch, no push.

Not in this plan (stated plainly): the RTL8852BE chip driver itself (firmware load, MAC/BB/RF tables, DMA
rings) and the I2C touchpad; both need the laptop in the loop and are listed as next work.
