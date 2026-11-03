//! Finding a boot-protocol keyboard in a USB configuration descriptor. The descriptor is a chain of
//! entries, each starting with its own length and a type: configuration (2), interface (4), HID (0x21)
//! and endpoint (5). Pure and host-tested; the bytes come from the device, so nothing here trusts them.

/// Where to talk to a boot keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootKeyboard {
    /// Value for Set Configuration.
    pub config: u8,
    pub interface: u8,
    /// Endpoint number (1 to 15) of the interrupt IN endpoint that sends the reports.
    pub endpoint: u8,
    pub max_packet: u16,
    /// Polling interval from the descriptor (units depend on the bus speed).
    pub interval: u8,
}

const DESC_CONFIG: u8 = 2;
const DESC_INTERFACE: u8 = 4;
const DESC_ENDPOINT: u8 = 5;
const CLASS_HID: u8 = 3;
const SUBCLASS_BOOT: u8 = 1;
const PROTOCOL_KEYBOARD: u8 = 1;

/// Walks a configuration descriptor (the whole chain, as returned for `wTotalLength` bytes) and returns
/// the first interface that is a boot keyboard together with its interrupt IN endpoint.
/// `None` when there is none, or when the chain is malformed (zero or overlong entry lengths).
pub fn find_boot_keyboard(config_desc: &[u8]) -> Option<BootKeyboard> {
    let d = config_desc;
    let mut i = 0usize;
    let mut config = 0u8;
    // The interface number while inside a boot-keyboard interface (its first alternate setting).
    let mut keyboard_interface: Option<u8> = None;
    while i < d.len() {
        let len = usize::from(d[i]);
        let kind = *d.get(i + 1)?;
        if len < 2 {
            return None; // would never advance
        }
        let e = d.get(i..i + len)?; // an entry that runs past the end is malformed
        match kind {
            DESC_CONFIG if len >= 9 => config = e[5],
            DESC_INTERFACE if len >= 9 => {
                let boot_keyboard = e[3] == 0 && e[5] == CLASS_HID && e[6] == SUBCLASS_BOOT && e[7] == PROTOCOL_KEYBOARD;
                keyboard_interface = boot_keyboard.then_some(e[2]);
            }
            DESC_ENDPOINT if len >= 7 => {
                // Interrupt (attributes 3), direction IN (bit 7 of the address).
                if let (Some(interface), true) = (keyboard_interface, e[2] & 0x80 != 0 && e[3] & 0b11 == 0b11) {
                    return Some(BootKeyboard {
                        config,
                        interface,
                        endpoint: e[2] & 0x0F,
                        max_packet: u16::from_le_bytes([e[4], e[5]]) & 0x07FF,
                        interval: e[6],
                    });
                }
            }
            _ => {}
        }
        i += len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real keyboard: configuration, boot-keyboard interface, HID descriptor, interrupt IN endpoint 1.
    const KEYBOARD: [u8; 34] = [
        0x09, 0x02, 0x22, 0x00, 0x01, 0x01, 0x00, 0xA0, 0x32, // configuration 1, one interface
        0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x01, 0x01, 0x00, // interface 0: HID, boot, keyboard
        0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x3F, 0x00, // HID descriptor
        0x07, 0x05, 0x81, 0x03, 0x08, 0x00, 0x0A, // endpoint 0x81: interrupt IN, 8 bytes, interval 10
    ];

    #[test]
    fn a_boot_keyboard_is_found() {
        assert_eq!(find_boot_keyboard(&KEYBOARD), Some(BootKeyboard { config: 1, interface: 0, endpoint: 1, max_packet: 8, interval: 10 }));
    }

    #[test]
    fn a_boot_mouse_is_not_a_keyboard() {
        let mut d = KEYBOARD;
        d[16] = 2; // protocol 2 = mouse
        assert_eq!(find_boot_keyboard(&d), None);
        d[16] = 0; // protocol 0 = none
        assert_eq!(find_boot_keyboard(&d), None);
        let mut d = KEYBOARD;
        d[15] = 0; // subclass 0 = no boot protocol
        assert_eq!(find_boot_keyboard(&d), None);
    }

    #[test]
    fn the_keyboard_is_found_behind_another_interface_and_its_out_endpoint_is_skipped() {
        let mut d = std::vec::Vec::new();
        d.extend_from_slice(&[0x09, 0x02, 0x00, 0x00, 0x02, 0x01, 0x00, 0xA0, 0x32]); // two interfaces
        d.extend_from_slice(&[0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x00, 0x00, 0x00]); // interface 0: HID, no boot protocol
        d.extend_from_slice(&[0x07, 0x05, 0x81, 0x03, 0x40, 0x00, 0x01]); // its endpoint: must not be returned
        d.extend_from_slice(&[0x09, 0x04, 0x01, 0x00, 0x02, 0x03, 0x01, 0x01, 0x00]); // interface 1: boot keyboard
        d.extend_from_slice(&[0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0x3F, 0x00]);
        d.extend_from_slice(&[0x07, 0x05, 0x02, 0x03, 0x08, 0x00, 0x0A]); // OUT endpoint 2 (LED report): skipped
        d.extend_from_slice(&[0x07, 0x05, 0x83, 0x03, 0x10, 0x00, 0x04]); // IN endpoint 3
        assert_eq!(find_boot_keyboard(&d), Some(BootKeyboard { config: 1, interface: 1, endpoint: 3, max_packet: 16, interval: 4 }));
    }

    #[test]
    fn a_bulk_endpoint_is_not_an_interrupt_endpoint() {
        let mut d = KEYBOARD;
        d[30] = 0x02; // bmAttributes: bulk
        assert_eq!(find_boot_keyboard(&d), None);
    }

    #[test]
    fn a_keyboard_without_an_endpoint_is_none() {
        assert_eq!(find_boot_keyboard(&KEYBOARD[..27]), None);
    }

    #[test]
    fn zero_length_and_overlong_entries_are_none() {
        let mut d = KEYBOARD;
        d[9] = 0; // interface entry of length 0 would loop forever
        assert_eq!(find_boot_keyboard(&d), None);
        let mut d = KEYBOARD;
        d[18] = 0xFF; // HID entry claims 255 bytes
        assert_eq!(find_boot_keyboard(&d), None);
        let mut d = KEYBOARD;
        d[0] = 1; // configuration entry shorter than a header
        assert_eq!(find_boot_keyboard(&d), None);
    }

    #[test]
    fn truncated_descriptors_and_garbage_never_panic() {
        for n in 0..KEYBOARD.len() {
            let _ = find_boot_keyboard(&KEYBOARD[..n]);
        }
        let mut seed = 0x1357_9BDF_2468_ACE0u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..5000 {
            let len = (next() % 64) as usize;
            let mut d: std::vec::Vec<u8> = (0..len).map(|_| next() as u8).collect();
            // Make some of them look like real chains so the walk goes deeper than the first byte.
            if next() % 2 == 0 && d.len() >= 9 {
                d[1] = DESC_CONFIG;
                d[0] = 9;
            }
            let _ = find_boot_keyboard(&d);
        }
    }
}
