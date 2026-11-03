# USB keyboard through xHCI: implementation plan

> **For agentic workers:** execute task by task; steps use checkboxes. Pure modules first (host tests), hardware last.

**Goal:** Aeon's shell accepts a USB keyboard, so a laptop with no PS/2 controller is usable.

**Architecture:** Three pure, host-tested modules (HID report to keys, USB descriptor search, xHCI ring and TRB logic) and one thin hardware module (`xhci.rs`) that owns the controller registers. The shell loop polls the driver once per pass, like it polls PS/2 today; no interrupts in the first version. No third-party crates: the `xhci` crate was last released in July 2023 and the driver is small enough to own.

**Tech Stack:** Rust `no_std`, QEMU `-device qemu-xhci -device usb-kbd` for tests.

**Spec:** `reports/Hobby Rust OS build checklist.md`, gap 3; notes in `research_notes/Hobby Rust OS build checklist/drivers_storage_net.md` (size estimate 2000 to 3000 lines is unsourced).

**Status (2026-11-03): tasks 1 to 5 are done and pass in QEMU; real hardware is untested. Added during the work: `xhcireg.rs` (register decoding) and `xhcictx.rs` (context builders), both pure and tested.**

## Global Constraints

- Both clippy gates clean with `-D warnings`; `undocumented_unsafe_blocks` is on, so every `unsafe` block needs a `SAFETY:` comment.
- Host tests: `cargo test --lib --target x86_64-pc-windows-gnu`; Miri script `bash tools/miri.sh` covers new pure modules that use `unsafe`.
- Unsigned image under 512 KiB (now 261,632 bytes).
- Never push. Commit with the Scrooge-777 identity.
- Keyboard first: boot protocol keyboards only (class 3, subclass 1, protocol 1). No hubs, no mass storage, no mouse in this plan.

## Review Focus

- A report with rollover error (`0x01` in all six key slots) must produce no keys.
- A key held down: the report repeats; the driver must not repeat it faster than the PS/2 path does (it should produce one `Key` per press, repeat handled by a timer later).
- A descriptor that lies about its own lengths (zero length, longer than the buffer) must not loop or panic.
- A controller that never clears its "not ready" bit or never hands over from the BIOS: time out and carry on without USB.
- A keyboard plugged in after boot, or unplugged: no hang.

### Task 1: HID boot report to keys (pure)

**Files:** Create `src/hidkbd.rs`; modify `src/lib.rs`.

**Interfaces:** Produces `pub struct HidKeyboard` with `pub fn feed(&mut self, report: &[u8], out: &mut impl FnMut(Key))`, using `crate::keymap::Key`.

- [ ] Tests first: a letter press gives `Key::Char(b'a')`, with shift `b'A'`; digits and punctuation with and without shift; Enter, Backspace, Tab, Esc, arrows, Home, End, PageUp, PageDown, Delete; Ctrl+C gives `Key::Interrupt`, Ctrl+L gives `Key::ClearScreen`, Ctrl+letter gives `Key::Ctrl`; a key held across two reports gives one event; two keys pressed in one report give two events in order; release gives nothing; error rollover (`01 01 01 01 01 01`) gives nothing and does not forget held keys; reports shorter than 8 bytes are ignored; Caps Lock toggles letter case.
- [ ] Implement `feed` with a copy of the previous 6 key slots; a usage code is "new" when it is absent from the previous report.
- [ ] Gate: unit tests, both clippy gates.

### Task 2: Find the boot keyboard in a USB configuration descriptor (pure)

**Files:** Create `src/usbdesc.rs`; modify `src/lib.rs`.

**Interfaces:** Produces `pub struct BootKeyboard { pub config: u8, pub interface: u8, pub endpoint: u8, pub max_packet: u16, pub interval: u8 }` and `pub fn find_boot_keyboard(config_desc: &[u8]) -> Option<BootKeyboard>`.

- [ ] Tests first: a real keyboard descriptor (9-byte configuration, 9-byte interface class 3 sub 1 protocol 1, 9-byte HID, 7-byte endpoint 0x81 interrupt) is found; a mouse (protocol 2) is not; a descriptor with a zero-length entry returns `None`; one cut off in the middle returns `None`; 5000 random byte strings never panic.
- [ ] Implement by walking `bLength` steps, bounds-checked.

### Task 3: xHCI rings and TRBs (pure)

**Files:** Create `src/xhcitrb.rs`; modify `src/lib.rs`.

**Interfaces:** Produces `Trb` (16 bytes, `#[repr(C)]`), constructors for Enable Slot, Address Device, Configure Endpoint, Setup/Data/Status stage, Normal and Link TRBs, `CommandRing` / `TransferRing` (enqueue with producer cycle bit, link TRB at the end that toggles it), `EventRing` (dequeue while the cycle bit matches).

- [ ] Tests first: TRB bit layout against the xHCI specification table values; enqueue wraps through the link TRB and flips the cycle bit; the event ring returns only entries with the expected cycle bit and flips it at wrap; a full ring refuses to enqueue.
- [ ] Implement over `[Trb; N]` slices so tests can use plain arrays.

### Task 4: The controller driver (hardware)

**Files:** Create `src/xhci.rs`; modify `src/lib.rs`, `src/main.rs` (init after PCI scan), `src/shell.rs` (poll in the key loop, `usb` command), `src/cmdhelp.rs`.

**Interfaces:** Produces `xhci::init()`, `xhci::poll_key() -> Option<Key>`, `xhci::summary()` for the `usb` command.

- [ ] Find PCI class 0x0C subclass 0x03 prog-if 0x30; map BAR0 (identity map exists; check it is uncached for MMIO).
- [ ] Take ownership from the BIOS through the USB Legacy Support extended capability, with a timeout.
- [ ] Reset, wait for "controller not ready" to clear, program DCBAA, command ring, event ring and scratchpad buffers, run.
- [ ] Per connected port: reset, read speed, Enable Slot, Address Device, read the device and configuration descriptors, `find_boot_keyboard`, Set Configuration, Set Protocol (boot), Set Idle, Configure Endpoint, queue interrupt transfers.
- [ ] `poll_key` drains the event ring, feeds each 8-byte report to `HidKeyboard`, requeues the transfer.
- [ ] DMA memory from the frame allocator; every wait has a timeout; any failure logs a warning and leaves PS/2 working.
- [ ] Gate: clippy, host tests, Miri script, boot groups `shell` and `cpu`.

### Task 5: QEMU test and real-PC instructions

**Files:** Modify `tests/boot_test.py` (new group `usbkbd`), `docs/HANDOFF.md`, `G:\Aeon\Test\AEON-TEST-GUIDE.md`.

- [ ] Group `usbkbd`: boot with `-device qemu-xhci -device usb-kbd`; wait for the shell; QMP `send-key` is delivered to the USB keyboard only (no PS/2 keyboard attached through `-device`); type `echo usb-ok`; expect the output. Check `usb` lists one keyboard.
- [ ] Guide entry for the owner: on the real PC run `usb` and `pci`, write down what they print; try typing.

### Real-hardware limits (state in the final report)

Only QEMU's xHCI is tested here. Laptops with the keyboard on an internal hub, controllers that need vendor quirks, and USB 3 ports that route through a hub are not covered. If the first real-PC test fails, the `usb` command and serial log say where.
