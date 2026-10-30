//! `BootInfo`: everything the firmware told us, copied into kernel-owned memory before
//! `ExitBootServices` so nothing depends on boot-services memory afterwards.

use crate::acpi::Rsdp;
use crate::memmap::{Region, Summary};
use crate::pixel::PixelFormat;

pub const MAX_REGIONS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecureBoot {
    /// Variable not readable (firmware without Secure Boot support).
    Unknown,
    Disabled,
    Enabled,
    /// Platform key not enrolled: Secure Boot is not enforcing even if the flag reads 1.
    SetupMode,
}

impl SecureBoot {
    pub fn name(self) -> &'static str {
        match self {
            SecureBoot::Unknown => "unknown",
            SecureBoot::Disabled => "disabled",
            SecureBoot::Enabled => "enabled",
            SecureBoot::SetupMode => "setup-mode",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FbInfo {
    pub base: u64,
    pub size: usize,
    pub width: usize,
    pub height: usize,
    /// Pixels (not bytes) per scan line.
    pub stride: usize,
    pub format: PixelFormat,
}

pub struct BootInfo {
    pub fw_vendor: [u8; 48],
    pub fw_vendor_len: usize,
    pub fw_revision: u32,
    pub uefi_revision: u32,
    pub image_base: u64,
    pub image_size: u64,
    pub cmdline: [u8; 128],
    pub cmdline_len: usize,
    pub fb: Option<FbInfo>,
    pub acpi_rsdp: u64,
    pub acpi: Rsdp,
    pub smbios: u64,
    pub secure_boot: SecureBoot,
    pub regions: [Region; MAX_REGIONS],
    pub region_count: usize,
    pub regions_dropped: usize,
    pub mem: Summary,
    pub tsc_hz: u64,
    /// Physical address of the firmware runtime-services table (valid after `ExitBootServices`).
    pub runtime_services: usize,
    /// 32-byte boot random seed (hash of firmware RNG, RDRAND and timing). Never logged or printed.
    pub seed: [u8; 32],
    /// How many independent sources went into `seed` (0 means the fixed fallback is in use).
    pub seed_sources: u8,
}

impl BootInfo {
    pub const fn new() -> Self {
        Self {
            fw_vendor: [0; 48],
            fw_vendor_len: 0,
            fw_revision: 0,
            uefi_revision: 0,
            image_base: 0,
            image_size: 0,
            cmdline: [0; 128],
            cmdline_len: 0,
            fb: None,
            acpi_rsdp: 0,
            acpi: Rsdp::Invalid,
            smbios: 0,
            secure_boot: SecureBoot::Unknown,
            regions: [Region::EMPTY; MAX_REGIONS],
            region_count: 0,
            regions_dropped: 0,
            mem: Summary {
                regions: 0,
                usable_pages: 0,
                reclaimable_pages: 0,
                acpi_pages: 0,
                runtime_pages: 0,
                mmio_pages: 0,
                reserved_pages: 0,
                ram_pages: 0,
                largest_usable_pages: 0,
                top_of_ram: 0,
            },
            tsc_hz: 0,
            runtime_services: 0,
            seed: [0; 32],
            seed_sources: 0,
        }
    }

    pub fn regions(&self) -> &[Region] {
        &self.regions[..self.region_count]
    }

    pub fn fw_vendor_str(&self) -> &str {
        core::str::from_utf8(&self.fw_vendor[..self.fw_vendor_len]).unwrap_or("?")
    }

    pub fn cmdline_str(&self) -> &str {
        core::str::from_utf8(&self.cmdline[..self.cmdline_len]).unwrap_or("?")
    }
}

impl Default for BootInfo {
    fn default() -> Self {
        Self::new()
    }
}
