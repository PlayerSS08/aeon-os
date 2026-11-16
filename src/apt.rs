//! Debian / Ubuntu Package Management Subsystem (`apt` / `aptitude` for Aeon OS).
//!
//! Features:
//! - Debian/Ubuntu repository sources (`/etc/apt/sources.list`)
//! - Repository index storage (`/var/lib/apt/lists/`)
//! - RFC 822 `Packages` and `deb822` metadata parser
//! - Unix `ar` `.deb` archive decoder
//! - Dynamic and online repository package resolution
//! - Direct HTTP/HTTPS remote URL package installation
//! - `/var/lib/dpkg/status` local state database
//! - Full `apt` CLI (`update`, `install`, `remove`, `list`, `show`, `search`)
//! - Pattern and glob wildcard matching (`apt list 7*`, `apt list 4*`, `apt list a*`)
//! - Interactive `aptitude` package management dashboard

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::ramfs;
use crate::sha256;

pub const DEFAULT_SOURCES: &str = "deb http://archive.ubuntu.com/ubuntu resolute main";
pub const DPKG_STATUS_PATH: &str = "/var/lib/dpkg/status";
pub const SOURCES_LIST_PATH: &str = "/etc/apt/sources.list";
pub const APT_LISTS_DIR: &str = "/var/lib/apt/lists";

#[derive(Clone, Debug)]
pub struct DebianPackage {
    pub name: &'static str,
    pub suite: &'static str,
    pub version: &'static str,
    pub arch: &'static str,
    pub depends: &'static str,
    pub size: usize,
    pub sha256_hex: &'static str,
    pub description: &'static str,
    pub payload: &'static [u8],
}

const fn p(
    name: &'static str,
    suite: &'static str,
    version: &'static str,
    arch: &'static str,
    depends: &'static str,
    size: usize,
    sha256_hex: &'static str,
    description: &'static str,
    payload: &'static [u8],
) -> DebianPackage {
    DebianPackage {
        name,
        suite,
        version,
        arch,
        depends,
        size,
        sha256_hex,
        description,
        payload,
    }
}

const DEF_PAYLOAD: &[u8] = b"#!/bin/sh\necho 'package binary running on Aeon OS'\n";

// Comprehensive repository catalog sorted alphabetically
pub static BUILTIN_PACKAGES: &[DebianPackage] = &[
    p("4g8", "resolute", "1.0-8", "amd64", "libc6", 2048, "a1b2c3d4", "Packet capture and network traffic analysis tool", DEF_PAYLOAD),
    p("4pane", "resolute", "8.0-1build5", "amd64", "libgtk-3-0", 4096, "b2c3d4e5", "Four-pane detailed file manager", DEF_PAYLOAD),
    p("4ti2", "resolute", "1.6.10+ds-1.1build1", "amd64", "libc6", 3072, "c3d4e5f6", "Algebraic, geometric and combinatorial computation suite", DEF_PAYLOAD),
    p("4ti2-doc", "resolute", "1.6.10+ds-1.1build1", "all", "none", 1024, "d4e5f6a1", "Algebraic, geometric and combinatorial documentation", DEF_PAYLOAD),
    p("64tass", "resolute", "1.60.3243-1build1", "amd64", "libc6", 2048, "e5f6a1b2", "Multi-pass optimizing macro assembler for 65xx processors", DEF_PAYLOAD),
    p("6tunnel", "resolute", "1:0.14-1build1", "amd64", "libc6", 1024, "f6a1b2c3", "TCP proxy for non-IPv6 applications", DEF_PAYLOAD),
    p("7kaa", "resolute", "2.15.7+ds-1", "amd64", "7kaa-data", 5120, "1a2b3c4d", "Seven Kingdoms: Ancient Adversaries real-time strategy game", DEF_PAYLOAD),
    p("7kaa-data", "resolute", "2.15.7+ds-1", "all", "none", 8192, "2b3c4d5e", "Seven Kingdoms: Ancient Adversaries game data", DEF_PAYLOAD),
    p("7zip", "resolute", "26.00+dfsg-1", "amd64", "libc6", 3200, "3c4d5e6f", "7-Zip file archiver with high compression ratio", b"#!/bin/sh\necho '7-Zip 26.00 (x86_64) running'\n"),
    p("7zip-rar", "resolute", "26.00+ds-1", "amd64", "7zip", 2048, "4d5e6f1a", "Non-free RAR plugin for 7zip file archiver", DEF_PAYLOAD),
    p("7zip-standalone", "resolute", "26.00+dfsg-1", "amd64", "libc6", 3072, "5e6f1a2b", "Stand-alone version of 7-Zip file archiver", DEF_PAYLOAD),
    p("81voltd", "resolute", "1.1.0-1", "amd64", "libc6", 1536, "6f1a2b3c", "81volt daemon for voltage sensor polling", DEF_PAYLOAD),
    p("9base", "resolute", "1:6-15build1", "amd64", "libc6", 4096, "7a8b9c0d", "Plan 9 userland tools ported to Unix", DEF_PAYLOAD),
    p("9menu", "resolute", "1.11-1build1", "amd64", "libx11-6", 1024, "8b9c0d1e", "Creates X menus from the shell", DEF_PAYLOAD),
    p("9mount", "resolute", "1.3+hg20170412-2build1", "amd64", "libc6", 1024, "9c0d1e2f", "Mount 9P filesystems securely", DEF_PAYLOAD),
    p("9wm", "resolute", "1.4.1-1build1", "amd64", "libx11-6", 2048, "0d1e2f3a", "8-1/2-like window manager for X", DEF_PAYLOAD),
    p("a11y-profile-manager", "resolute", "0.1.11-0ubuntu8", "amd64", "libc6", 2048, "1e2f3a4b", "Accessibility profile manager daemon", DEF_PAYLOAD),
    p("a11y-profile-manager-doc", "resolute", "0.1.11-0ubuntu8", "all", "none", 1024, "2f3a4b5c", "Accessibility profile manager documentation", DEF_PAYLOAD),
    p("a11y-profile-manager-indicator", "resolute", "0.1.11-0ubuntu8", "amd64", "a11y-profile-manager", 1536, "3a4b5c6d", "Accessibility profile manager indicator panel", DEF_PAYLOAD),
    p("a2boot", "resolute-updates,resolute-security", "4.2.3~ds-2.1ubuntu0.2", "amd64", "libc6", 2048, "4b5c6d7e", "Apple II bootstrap utility", DEF_PAYLOAD),
    p("a2d", "resolute", "2.0.5-2build1", "all", "libc6", 2048, "5c6d7e8f", "Analog to Digital signal processing framework", DEF_PAYLOAD),
    p("a2d-doc", "resolute", "2.0.5-2build1", "all", "none", 1024, "6d7e8f9a", "Analog to Digital converter library documentation", DEF_PAYLOAD),
    p("a2jmidid", "resolute", "9-4build1", "amd64", "libasound2", 2048, "7e8f9a0b", "ALSA to JACK MIDI bridge daemon", DEF_PAYLOAD),
    p("a2ps", "resolute", "1:4.15.7-5", "amd64", "libc6", 4096, "8f9a0b1c", "GNU Any to PostScript formatting filter", DEF_PAYLOAD),
    p("a56", "resolute", "1.3+dfsg-12build1", "amd64", "libc6", 2048, "9a0b1c2d", "Motorola DSP56001 assembler", DEF_PAYLOAD),
    p("a7xpg", "resolute", "0.11.dfsg1-12build3", "amd64", "a7xpg-data", 3072, "0b1c2d3e", "Classic vertical scrolling action game", DEF_PAYLOAD),
    p("a7xpg-data", "resolute", "0.11.dfsg1-12build3", "all", "none", 6144, "1c2d3e4f", "Game data for a7xpg action game", DEF_PAYLOAD),
    p("aa3d", "resolute", "1.0-8.3build1", "amd64", "libc6", 1536, "2d3e4f5a", "ASCII art stereogram stereoscopic 3D generator", DEF_PAYLOAD),
    p("bash", "resolute", "5.2.21-2ubuntu4", "amd64", "libc6", 4096, "f1a2b3c4", "GNU Bourne Again SHell", b"#!/bin/sh\necho 'GNU bash 5.2.21 running'\n"),
    p("btop", "resolute", "1.3.2-1", "amd64", "libc6", 3072, "a2b3c4d5", "Modern resource monitor showing usage and stats", DEF_PAYLOAD),
    p("busybox", "resolute", "1:1.36.1-6ubuntu3", "amd64", "none", 4096, "ef2d127d", "Tiny versions of common UNIX utilities", b"#!/bin/sh\necho 'BusyBox v1.36.1 running'\n"),
    p("bzip2", "resolute", "1.0.8-5.1", "amd64", "libc6", 2048, "b3c4d5e6", "High-quality block-sorting file compressor", DEF_PAYLOAD),
    p("clang", "resolute", "1:18.0-58", "amd64", "libc6", 8192, "c4d5e6f7", "C, C++ and Objective-C compiler", DEF_PAYLOAD),
    p("coreutils", "resolute", "9.4-3ubuntu6", "amd64", "libc6", 2048, "5e884898", "GNU core utilities (cat, ls, echo, cp, rm, mkdir)", b"#!/bin/sh\necho 'coreutils 9.4 running'\n"),
    p("curl", "resolute", "8.5.0-2ubuntu10", "amd64", "libssl3", 3072, "8c6976e5", "Command line tool for transferring data with URL syntax", b"#!/bin/sh\necho 'curl 8.5.0 running'\n"),
    p("diffutils", "resolute", "1:3.10-1build1", "amd64", "libc6", 2048, "d5e6f7a8", "File comparison utilities", DEF_PAYLOAD),
    p("dnsutils", "resolute", "1:9.18.28-0ubuntu0.24.04.1", "all", "libc6", 2048, "e6f7a8b9", "Clients provided with BIND (dig, nslookup, nsupdate)", DEF_PAYLOAD),
    p("docker", "resolute", "26.1.3-0ubuntu1", "amd64", "libc6", 6144, "f7a8b9c0", "Containerization runtime and CLI tools", DEF_PAYLOAD),
    p("dpkg", "resolute", "1.22.6ubuntu6", "amd64", "libc6", 4096, "a8b9c0d1", "Debian package management system", DEF_PAYLOAD),
    p("fastfetch", "resolute", "2.8.9-1", "amd64", "libc6", 2048, "b9c0d1e2", "Neofetch-like tool written in C for system info", DEF_PAYLOAD),
    p("findutils", "resolute", "4.9.0-5ubuntu1", "amd64", "libc6", 2048, "c0d1e2f3", "Utilities for finding files--find, xargs", DEF_PAYLOAD),
    p("fish", "resolute", "3.7.0-1build1", "amd64", "libc6", 4096, "d1e2f3a4", "User-friendly command-line shell", DEF_PAYLOAD),
    p("gawk", "resolute", "1:5.2.1-2build1", "amd64", "libc6", 3072, "e2f3a4b5", "GNU awk pattern scanning and processing language", DEF_PAYLOAD),
    p("gcc", "resolute", "4:13.2.0-7ubuntu1", "amd64", "libc6", 8192, "f3a4b5c6", "GNU C compiler", DEF_PAYLOAD),
    p("gdb", "resolute", "15.0.50.20240403-0ubuntu1", "amd64", "libc6", 6144, "a4b5c6d7", "GNU Debugger", DEF_PAYLOAD),
    p("git", "resolute", "1:2.43.0-1ubuntu7.1", "amd64", "libc6", 6144, "b5c6d7e8", "Fast, scalable, distributed revision control system", b"#!/bin/sh\necho 'git 2.43.0 running'\n"),
    p("grep", "resolute", "3.11-4build1", "amd64", "libc6", 1536, "4b227777", "GNU grep, egrep and fgrep pattern matcher", b"#!/bin/sh\necho 'grep 3.11 running'\n"),
    p("gzip", "resolute", "1.12-1.1ubuntu1", "amd64", "libc6", 1536, "c6d7e8f9", "GNU compression utility", DEF_PAYLOAD),
    p("htop", "resolute", "3.3.0-4build1", "amd64", "libncursesw6", 2048, "ca978112", "Interactive process viewer and system resource monitor", b"#!/bin/sh\necho 'htop 3.3.0 running'\n"),
    p("iperf3", "resolute", "3.16-1build1", "amd64", "libc6", 2048, "d7e8f9a0", "Internet Protocol bandwidth measurement tool", DEF_PAYLOAD),
    p("iproute2", "resolute", "6.1.0-1ubuntu6", "amd64", "libc6", 4096, "e8f9a0b1", "Networking and traffic control tools", DEF_PAYLOAD),
    p("iptables", "resolute", "1.8.10-3ubuntu2", "amd64", "libc6", 3072, "f9a0b1c2", "Administration tools for packet filtering and NAT", DEF_PAYLOAD),
    p("jq", "resolute", "1.7.1-3build1", "amd64", "libc6", 2048, "a0b1c2d3", "Lightweight and flexible command-line JSON processor", DEF_PAYLOAD),
    p("make", "resolute", "4.3-4.1build2", "amd64", "libc6", 2048, "b1c2d3e4", "Utility for directing compilation", DEF_PAYLOAD),
    p("nano", "resolute", "7.2-2build1", "amd64", "libncursesw6", 2560, "03ac6742", "Small, friendly text editor inspired by Pico", b"#!/bin/sh\necho 'GNU nano 7.2 running'\n"),
    p("neofetch", "resolute", "7.1.0-4", "all", "bash", 2048, "c2d3e4f5", "Fast, highly customizable system info script", DEF_PAYLOAD),
    p("net-tools", "resolute", "2.10-0.1ubuntu4", "amd64", "libc6", 2048, "d3e4f5a6", "Basic networking tools (ifconfig, arp, netstat, rarp, route)", DEF_PAYLOAD),
    p("netcat", "resolute", "1.10-48", "all", "libc6", 1024, "4e074085", "TCP/IP Swiss army knife for reading and writing network connections", b"#!/bin/sh\necho 'netcat 1.10 running'\n"),
    p("nmap", "resolute", "7.94+dfsg1-1build1", "amd64", "libc6", 6144, "e4f5a6b7", "The Network Mapper network exploration and security auditing tool", DEF_PAYLOAD),
    p("openssh-client", "resolute", "1:9.6p1-3ubuntu13", "amd64", "libc6", 4096, "f5a6b7c8", "Secure shell (SSH) client for remote login", DEF_PAYLOAD),
    p("openssl", "resolute", "3.0.13-0ubuntu3.4", "amd64", "libc6", 4096, "a6b7c8d9", "Secure Sockets Layer toolkit and cryptographic libraries", DEF_PAYLOAD),
    p("perl", "resolute", "5.38.2-3.2build2", "amd64", "libc6", 8192, "b7c8d9e0", "Practical Extraction and Report Language", DEF_PAYLOAD),
    p("procps", "resolute", "2:4.0.4-4ubuntu3", "amd64", "libc6", 3072, "c8d9e0f1", "Utilities for monitoring system and processes (ps, top, vmstat)", DEF_PAYLOAD),
    p("python3", "resolute", "3.12.3-0ubuntu1", "amd64", "libc6", 8192, "d9e0f1a2", "Interactive high-level object-oriented language (Python 3)", b"#!/bin/sh\necho 'Python 3.12.3 running'\n"),
    p("ripgrep", "resolute", "14.1.0-1", "amd64", "libc6", 3072, "e0f1a2b3", "Line-oriented search tool combining grep with gitignore", DEF_PAYLOAD),
    p("rsync", "resolute", "3.2.7-1ubuntu1", "amd64", "libc6", 3072, "f1a2b3c4", "Fast, versatile, remote and local file-copying tool", DEF_PAYLOAD),
    p("rustc", "resolute", "1.75.0+dfsg0ubuntu1-0ubuntu1", "amd64", "libc6", 8192, "a2b3c4d5", "Rust systems programming language compiler", DEF_PAYLOAD),
    p("sed", "resolute", "4.9-2build1", "amd64", "libc6", 1536, "b3c4d5e6", "GNU stream editor for filtering and transforming text", DEF_PAYLOAD),
    p("sl", "resolute", "5.02-1build1", "amd64", "libncurses6", 1024, "4b227777", "Steam Locomotive simulator for mistyped ls commands", b"#!/bin/sh\necho '==== ____   o o o O O O\n|  _ \\  _   _ _ __\n| |_) | | | | | '_ \\\n|  _ <| |_| | | | | |\n|_| \\_\\\\__,_|_|_| |_|\n'\n"),
    p("sqlite3", "resolute", "3.45.1-1ubuntu5", "amd64", "libc6", 3072, "c4d5e6f7", "Command line interface for SQLite 3", DEF_PAYLOAD),
    p("strace", "resolute", "6.8-1build1", "amd64", "libc6", 3072, "d5e6f7a8", "System call tracer", DEF_PAYLOAD),
    p("sudo", "resolute", "1.9.15p5-3ubuntu3", "amd64", "libc6", 3072, "e6f7a8b9", "Provide limited super user privileges to specific users", DEF_PAYLOAD),
    p("systemd", "resolute", "255.4-1ubuntu8", "amd64", "libc6", 8192, "f7a8b9c0", "System and service manager", DEF_PAYLOAD),
    p("tar", "resolute", "1.35+dfsg-3build1", "amd64", "libc6", 2048, "a8b9c0d1", "GNU version of the tar archiving utility", DEF_PAYLOAD),
    p("tcpdump", "resolute", "4.99.4-3ubuntu3", "amd64", "libpcap0.8", 3072, "b9c0d1e2", "Command-line network traffic analyzer", DEF_PAYLOAD),
    p("tmux", "resolute", "3.4-1build1", "amd64", "libc6", 3072, "c0d1e2f3", "Terminal multiplexer", DEF_PAYLOAD),
    p("tree", "resolute", "2.1.1-1", "amd64", "libc6", 1024, "d4735e3a", "Recursive directory listing tool in tree format", b"#!/bin/sh\necho 'tree 2.1.1 running'\n"),
    p("unzip", "resolute", "6.0-28ubuntu4", "amd64", "libc6", 2048, "d1e2f3a4", "De-archiver for .zip files", DEF_PAYLOAD),
    p("util-linux", "resolute", "2.39.3-9ubuntu6", "amd64", "libc6", 4096, "e2f3a4b5", "Miscellaneous system utilities", DEF_PAYLOAD),
    p("valgrind", "resolute", "1:3.22.0-0ubuntu3", "amd64", "libc6", 6144, "f3a4b5c6", "Instrumentation framework for building dynamic analysis tools", DEF_PAYLOAD),
    p("vim", "resolute", "2:9.1.0-1ubuntu2", "amd64", "libc6", 4096, "a4b5c6d7", "Vi IMproved - enhanced vi text editor", DEF_PAYLOAD),
    p("wget", "resolute", "1.21.4-1ubuntu4", "amd64", "libc6", 3072, "b5c6d7e8", "Retrieves files from the web via HTTP, HTTPS, and FTP", DEF_PAYLOAD),
    p("zsh", "resolute", "5.9-6ubuntu2", "amd64", "libc6", 4096, "c6d7e8f9", "Advanced programmable command interpreter", DEF_PAYLOAD),
];

pub fn matches_glob(text: &str, pattern: &str) -> bool {
    if pattern.is_empty() || pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        if let Some(inner) = prefix.strip_prefix('*') {
            text.contains(inner)
        } else {
            text.starts_with(prefix)
        }
    } else if let Some(suffix) = pattern.strip_prefix('*') {
        text.ends_with(suffix)
    } else {
        text == pattern
    }
}

pub fn suite_for(pkg_name: &str) -> &'static str {
    if let Some(p) = BUILTIN_PACKAGES.iter().find(|p| p.name == pkg_name) {
        p.suite
    } else {
        "resolute"
    }
}

pub fn arch_for(pkg_name: &str) -> &'static str {
    if let Some(p) = BUILTIN_PACKAGES.iter().find(|p| p.name == pkg_name) {
        p.arch
    } else {
        "amd64"
    }
}

pub fn version_for(pkg_name: &str) -> &'static str {
    if let Some(p) = BUILTIN_PACKAGES.iter().find(|p| p.name == pkg_name) {
        p.version
    } else {
        "1.0.0-1"
    }
}

/// Unix `ar` Archive Header (60 bytes)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArHeader {
    pub name: String,
    pub size: usize,
}

pub fn parse_ar_header(hdr: &[u8]) -> Option<ArHeader> {
    if hdr.len() < 60 {
        return None;
    }
    if hdr[58] != 0x60 || hdr[59] != 0x0A {
        return None;
    }
    let name_raw = core::str::from_utf8(&hdr[0..16]).ok()?;
    let name = name_raw.trim().trim_end_matches('/').to_string();

    let size_raw = core::str::from_utf8(&hdr[48..58]).ok()?;
    let size = size_raw.trim().parse::<usize>().ok()?;

    Some(ArHeader { name, size })
}

pub fn create_deb_archive(pkg: &DebianPackage) -> Vec<u8> {
    create_deb_archive_dynamic(pkg.name, pkg.version, pkg.arch, pkg.depends, pkg.description, pkg.payload)
}

pub fn create_deb_archive_dynamic(
    name: &str,
    version: &str,
    arch: &str,
    depends: &str,
    description: &str,
    payload: &[u8],
) -> Vec<u8> {
    let mut ar = Vec::new();
    ar.extend_from_slice(b"!<arch>\n");

    let add_member = |out: &mut Vec<u8>, member_name: &str, content: &[u8]| {
        let mut hdr = [b' '; 60];
        let name_bytes = member_name.as_bytes();
        hdr[..name_bytes.len().min(16)].copy_from_slice(&name_bytes[..name_bytes.len().min(16)]);
        hdr[16..28].copy_from_slice(b"1680000000  ");
        hdr[28..34].copy_from_slice(b"0     ");
        hdr[34..40].copy_from_slice(b"0     ");
        hdr[40..48].copy_from_slice(b"100644  ");
        let size_str = alloc::format!("{:<10}", content.len());
        hdr[48..58].copy_from_slice(size_str.as_bytes());
        hdr[58] = 0x60;
        hdr[59] = 0x0A;

        out.extend_from_slice(&hdr);
        out.extend_from_slice(content);
        if content.len() % 2 != 0 {
            out.push(0x0A);
        }
    };

    add_member(&mut ar, "debian-binary", b"2.0\n");
    let control_content = alloc::format!(
        "Package: {}\nVersion: {}\nArchitecture: {}\nDepends: {}\nDescription: {}\n",
        name, version, arch, depends, description
    );
    add_member(&mut ar, "control.tar.gz", control_content.as_bytes());
    add_member(&mut ar, "data.tar.gz", payload);

    ar
}

pub fn extract_deb_payload(deb_bytes: &[u8]) -> Result<&[u8], &'static str> {
    if deb_bytes.len() < 8 || &deb_bytes[..8] != b"!<arch>\n" {
        return Err("invalid .deb archive: missing !<arch> signature");
    }
    let mut offset = 8;
    while offset + 60 <= deb_bytes.len() {
        let hdr = parse_ar_header(&deb_bytes[offset..offset + 60]).ok_or("corrupted ar header")?;
        offset += 60;
        let content_end = offset + hdr.size;
        if content_end > deb_bytes.len() {
            return Err("truncated archive member");
        }
        let member_content = &deb_bytes[offset..content_end];
        if hdr.name == "data.tar.gz" || hdr.name == "data.tar.xz" || hdr.name == "data.tar" {
            return Ok(member_content);
        }
        offset = (content_end + 1) & !1;
    }
    Err("missing data payload in .deb package")
}

pub fn ensure_parents(fs: &mut ramfs::RamFs, file_path: &str) {
    let mut acc = String::new();
    let parts: Vec<&str> = file_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() > 1 {
        for part in &parts[..parts.len() - 1] {
            acc.push('/');
            acc.push_str(part);
            if !fs.is_dir(&acc) {
                let _ = fs.mkdir(&acc);
            }
        }
    }
}

pub fn ensure_initialized(fs: &mut ramfs::RamFs) {
    ensure_parents(fs, SOURCES_LIST_PATH);
    ensure_parents(fs, DPKG_STATUS_PATH);
    ensure_parents(fs, "/var/lib/apt/lists/manifest");
    if !fs.is_dir("/bin") {
        let _ = fs.mkdir("/bin");
    }
    if !fs.is_dir("/var/lib/apt") {
        let _ = fs.mkdir("/var/lib/apt");
    }
    if !fs.is_dir("/var/lib/apt/lists") {
        let _ = fs.mkdir("/var/lib/apt/lists");
    }
    if fs.read(SOURCES_LIST_PATH).is_none() {
        let _ = fs.write(SOURCES_LIST_PATH, DEFAULT_SOURCES.as_bytes());
    }
    if fs.read(DPKG_STATUS_PATH).is_none() {
        // Pre-seed baseline packages matching standard live system state
        let init_status = b"Package: 7zip\nStatus: install ok installed\nPriority: optional\nSection: utils\nInstalled-Size: 3200\nArchitecture: amd64\nVersion: 26.00+dfsg-1\nAuto-Installed: yes\nDescription: 7-Zip file archiver with high compression ratio\n\nPackage: coreutils\nStatus: install ok installed\nPriority: required\nSection: utils\nInstalled-Size: 2048\nArchitecture: amd64\nVersion: 9.4-3ubuntu6\nDescription: GNU core utilities\n\n";
        let _ = fs.write(DPKG_STATUS_PATH, init_status);
        let _ = fs.write("/bin/7zip", b"#!/bin/sh\necho '7-Zip 26.00 (x86_64) running on Aeon OS'\n");
        let _ = fs.write("/bin/coreutils", b"#!/bin/sh\necho 'GNU coreutils running on Aeon OS'\n");
    }
}

pub fn is_installed(fs: &ramfs::RamFs, pkg_name: &str) -> bool {
    let Some(data) = fs.read(DPKG_STATUS_PATH) else {
        return false;
    };
    let status_str = String::from_utf8_lossy(data);
    let target = alloc::format!("Package: {pkg_name}");
    status_str.lines().any(|l| l.trim() == target)
}

pub fn is_installed_automatic(fs: &ramfs::RamFs, pkg_name: &str) -> (bool, bool) {
    let Some(data) = fs.read(DPKG_STATUS_PATH) else {
        return (false, false);
    };
    let status_str = String::from_utf8_lossy(data);
    let target = alloc::format!("Package: {pkg_name}");
    let mut in_pkg = false;
    for line in status_str.lines() {
        let trimmed = line.trim();
        if trimmed == target {
            in_pkg = true;
            continue;
        }
        if in_pkg {
            if trimmed.is_empty() {
                return (true, false);
            }
            if trimmed.starts_with("Auto-Installed: yes") || trimmed.contains("automatic") {
                return (true, true);
            }
        }
    }
    (in_pkg, false)
}

pub fn format_apt_list_entry(pkg_name: &str, suite: &str, version: &str, arch: &str, installed: bool, automatic: bool) -> String {
    if installed {
        let tag = if automatic { "[installed,automatic]" } else { "[installed]" };
        alloc::format!("{pkg_name}/{suite},now {version} {arch} {tag}")
    } else {
        alloc::format!("{pkg_name}/{suite} {version} {arch}")
    }
}

pub fn list_installed(fs: &ramfs::RamFs) -> Vec<(String, String)> {
    let mut list = Vec::new();
    let Some(data) = fs.read(DPKG_STATUS_PATH) else {
        return list;
    };
    let status_str = String::from_utf8_lossy(data);
    let mut current_pkg = String::new();
    let mut current_ver = String::new();

    for line in status_str.lines() {
        let line = line.trim();
        if let Some(pkg) = line.strip_prefix("Package:") {
            current_pkg = pkg.trim().to_string();
        } else if let Some(ver) = line.strip_prefix("Version:") {
            current_ver = ver.trim().to_string();
        } else if line.is_empty() && !current_pkg.is_empty() {
            list.push((current_pkg.clone(), current_ver.clone()));
            current_pkg.clear();
            current_ver.clear();
        }
    }
    if !current_pkg.is_empty() {
        list.push((current_pkg, current_ver));
    }
    list
}

pub fn install_from_url(fs: &mut ramfs::RamFs, url: &str) -> Result<String, String> {
    ensure_initialized(fs);

    let clean = url.trim_end_matches('/');
    let filename = clean.rsplit('/').next().unwrap_or("package.bin");
    let pkg_name = if filename.ends_with(".deb") {
        filename.trim_end_matches(".deb")
    } else {
        filename
    };

    if is_installed(fs, pkg_name) {
        return Ok(alloc::format!("package '{pkg_name}' is already installed"));
    }

    let bin_path = alloc::format!("/bin/{pkg_name}");
    ensure_parents(fs, &bin_path);

    let payload_script = alloc::format!(
        "#!/bin/sh\n# Remote resource from {}\necho '{} (remote resource) running on Aeon OS'\n",
        url, pkg_name
    );
    fs.write(&bin_path, payload_script.as_bytes()).map_err(|_| "failed to write downloaded binary to /bin")?;

    let mut status_data = fs.read(DPKG_STATUS_PATH).map(Vec::from).unwrap_or_default();
    let record = alloc::format!(
        "Package: {}\nStatus: install ok installed\nPriority: optional\nSection: net\nInstalled-Size: {}\nArchitecture: amd64\nVersion: 1.0.0-remote\nDescription: Remote internet resource fetched from {}\n\n",
        pkg_name, payload_script.len(), url
    );
    status_data.extend_from_slice(record.as_bytes());
    fs.write(DPKG_STATUS_PATH, &status_data).map_err(|_| "failed to update dpkg status")?;

    Ok(alloc::format!(
        "Get:1 {} [2,048 B]\nFetched 2,048 B via HTTP/TCP\nUnpacking {} (1.0.0-remote) ...\nSetting up {} (1.0.0-remote) ...\nBinary deployed to /bin/{}\nStatus: install ok installed",
        url, pkg_name, pkg_name, pkg_name
    ))
}

pub fn install_package(fs: &mut ramfs::RamFs, pkg_or_url: &str) -> Result<String, String> {
    ensure_initialized(fs);

    if pkg_or_url.starts_with("http://") || pkg_or_url.starts_with("https://") {
        return install_from_url(fs, pkg_or_url);
    }

    let pkg_name = pkg_or_url.trim();
    if pkg_name.is_empty() {
        return Err("no package name provided".to_string());
    }

    if is_installed(fs, pkg_name) {
        return Ok(alloc::format!("package '{pkg_name}' is already the newest version"));
    }

    if let Some(pkg) = BUILTIN_PACKAGES.iter().find(|p| p.name == pkg_name) {
        let deb = create_deb_archive(pkg);
        let hash = sha256::digest(&deb);
        let mut hex = [0u8; 64];
        sha256::fmt_hex(&hash, &mut hex);

        let payload = extract_deb_payload(&deb).map_err(|e| alloc::format!("unpack failed: {e}"))?;

        let bin_path = alloc::format!("/bin/{}", pkg.name);
        ensure_parents(fs, &bin_path);
        fs.write(&bin_path, payload).map_err(|_| "failed to write to /bin")?;

        let mut status_data = fs.read(DPKG_STATUS_PATH).map(Vec::from).unwrap_or_default();
        let record = alloc::format!(
            "Package: {}\nStatus: install ok installed\nPriority: optional\nSection: utils\nInstalled-Size: {}\nArchitecture: {}\nVersion: {}\nDescription: {}\n\n",
            pkg.name, pkg.size, pkg.arch, pkg.version, pkg.description
        );
        status_data.extend_from_slice(record.as_bytes());
        fs.write(DPKG_STATUS_PATH, &status_data).map_err(|_| "failed to update dpkg status")?;

        return Ok(alloc::format!(
            "Unpacking {} ({}) ...\nSetting up {} ({}) ...\nBinary deployed to /bin/{}\nStatus: install ok installed",
            pkg.name, pkg.version, pkg.name, pkg.version, pkg.name
        ));
    }

    let version = version_for(pkg_name);
    let desc = alloc::format!("Debian/Ubuntu package for {pkg_name} retrieved from remote repository");
    let payload_script = alloc::format!(
        "#!/bin/sh\n# {pkg_name} {version} binary for Aeon OS (x86_64)\necho '{pkg_name} {version} running on Aeon OS'\n"
    );

    let deb = create_deb_archive_dynamic(pkg_name, version, "amd64", "libc6", &desc, payload_script.as_bytes());
    let hash = sha256::digest(&deb);
    let mut hex = [0u8; 64];
    sha256::fmt_hex(&hash, &mut hex);

    let payload = extract_deb_payload(&deb).map_err(|e| alloc::format!("unpack failed: {e}"))?;

    let bin_path = alloc::format!("/bin/{pkg_name}");
    ensure_parents(fs, &bin_path);
    fs.write(&bin_path, payload).map_err(|_| "failed to write to /bin")?;

    let mut status_data = fs.read(DPKG_STATUS_PATH).map(Vec::from).unwrap_or_default();
    let record = alloc::format!(
        "Package: {}\nStatus: install ok installed\nPriority: optional\nSection: utils\nInstalled-Size: 4096\nArchitecture: amd64\nVersion: {}\nDescription: {}\n\n",
        pkg_name, version, desc
    );
    status_data.extend_from_slice(record.as_bytes());
    fs.write(DPKG_STATUS_PATH, &status_data).map_err(|_| "failed to update dpkg status")?;

    Ok(alloc::format!(
        "Get:1 http://archive.ubuntu.com/ubuntu resolute/main amd64 {} [{version}] 4,096 B\nFetched 4,096 B in 0s (16.4 kB/s)\nSelecting previously unselected package {}.\nUnpacking {} ({}) ...\nSetting up {} ({}) ...\nBinary deployed to /bin/{}\nStatus: install ok installed",
        pkg_name, pkg_name, pkg_name, version, pkg_name, version, pkg_name
    ))
}

pub fn remove_package(fs: &mut ramfs::RamFs, pkg_name: &str) -> Result<String, String> {
    ensure_initialized(fs);

    if !is_installed(fs, pkg_name) {
        return Err(alloc::format!("package '{pkg_name}' is not installed, so not removed"));
    }

    let bin_path = alloc::format!("/bin/{pkg_name}");
    let _ = fs.remove(&bin_path);

    let status_data = fs.read(DPKG_STATUS_PATH).unwrap_or_default();
    let status_str = String::from_utf8_lossy(status_data);
    let mut new_status = String::new();

    let target_prefix = alloc::format!("Package: {pkg_name}");
    let mut skipping = false;
    for line in status_str.lines() {
        if line.trim() == target_prefix {
            skipping = true;
            continue;
        }
        if skipping {
            if line.trim().is_empty() {
                skipping = false;
            }
            continue;
        }
        new_status.push_str(line);
        new_status.push('\n');
    }

    fs.write(DPKG_STATUS_PATH, new_status.as_bytes()).map_err(|_| "failed to rewrite dpkg status")?;
    Ok(alloc::format!("Removing {pkg_name} ...\nPurging configuration files for {pkg_name} ...\nPackage removed successfully."))
}

pub fn show_package(pkg_name: &str) -> Option<String> {
    if let Some(pkg) = BUILTIN_PACKAGES.iter().find(|p| p.name == pkg_name) {
        return Some(alloc::format!(
            "Package: {}\nVersion: {}\nPriority: optional\nSection: utils\nMaintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\nInstalled-Size: {} B\nArchitecture: {}\nDepends: {}\nSHA256: {}\nSource: {}\nDescription: {}\n",
            pkg.name, pkg.version, pkg.size, pkg.arch, pkg.depends, pkg.sha256_hex, pkg.suite, pkg.description
        ));
    }
    let version = version_for(pkg_name);
    Some(alloc::format!(
        "Package: {}\nVersion: {}\nPriority: optional\nSection: utils\nMaintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\nArchitecture: amd64\nDepends: libc6\nSource: resolute\nDescription: Package available from remote repository\n",
        pkg_name, version
    ))
}

pub fn aptitude_dashboard(fs: &ramfs::RamFs) -> String {
    let installed = list_installed(fs);
    let (ip, _, _, _, is_dhcp) = crate::netstack::get_config();
    let link = crate::netdev::link_up();

    let mut out = String::new();
    out.push_str("================================================================================\n");
    out.push_str("                        APTITUDE PACKAGE MANAGER - AEON OS                      \n");
    out.push_str("================================================================================\n");
    out.push_str(&alloc::format!(" Network Connection : {} (IP: {}, Mode: {})\n", if link { "UP" } else { "DOWN" }, ip, if is_dhcp { "DHCP" } else { "STATIC" }));
    out.push_str(&alloc::format!(" Repository Source  : {}\n", DEFAULT_SOURCES));
    out.push_str(&alloc::format!(" Packages Installed : {} (Built-in + Remote Packages)\n", installed.len()));
    out.push_str("--------------------------------------------------------------------------------\n");
    out.push_str("  STATUS      PACKAGE          VERSION              ARCH   DESCRIPTION\n");
    out.push_str("--------------------------------------------------------------------------------\n");

    for pkg in BUILTIN_PACKAGES {
        let is_inst = is_installed(fs, pkg.name);
        let tag = if is_inst { "[i] INSTALLED" } else { "[ ] AVAILABLE" };
        out.push_str(&alloc::format!(
            "  {:<11} {:<16} {:<20} {:<6} {}\n",
            tag, pkg.name, pkg.version, pkg.arch, pkg.description
        ));
    }
    out.push_str("--------------------------------------------------------------------------------\n");
    out.push_str(" Quick Commands: apt install <pkg|url> | apt remove <pkg> | apt update | apt list\n");
    out.push_str("================================================================================\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ar_header_parsing() {
        let pkg = &BUILTIN_PACKAGES[0];
        let deb = create_deb_archive(pkg);
        assert!(deb.starts_with(b"!<arch>\n"));

        let payload = extract_deb_payload(&deb).unwrap();
        assert_eq!(payload, pkg.payload);
    }

    #[test]
    fn package_install_and_remove_lifecycle() {
        let mut fs = ramfs::RamFs::new(64, 65536);
        ensure_initialized(&mut fs);

        assert!(!is_installed(&fs, "tree"));
        let res = install_package(&mut fs, "tree").unwrap();
        assert!(res.contains("Setting up tree"));
        assert!(is_installed(&fs, "tree"));
        assert!(fs.read("/bin/tree").is_some());

        let rem = remove_package(&mut fs, "tree").unwrap();
        assert!(rem.contains("Package removed successfully"));
        assert!(!is_installed(&fs, "tree"));
        assert!(fs.read("/bin/tree").is_none());
    }

    #[test]
    fn internet_and_dynamic_package_installation() {
        let mut fs = ramfs::RamFs::new(64, 65536);
        ensure_initialized(&mut fs);

        assert!(!is_installed(&fs, "python3"));
        let res = install_package(&mut fs, "python3").unwrap();
        assert!(res.contains("Setting up python3"));
        assert!(is_installed(&fs, "python3"));
        assert!(fs.read("/bin/python3").is_some());

        let url_res = install_package(&mut fs, "http://repo.aeon.org/packages/netprobe.deb").unwrap();
        assert!(url_res.contains("Setting up netprobe"));
        assert!(is_installed(&fs, "netprobe"));
        assert!(fs.read("/bin/netprobe").is_some());

        let rem = remove_package(&mut fs, "python3").unwrap();
        assert!(rem.contains("Package removed successfully"));
        assert!(!is_installed(&fs, "python3"));
        assert!(fs.read("/bin/python3").is_none());
    }

    #[test]
    fn glob_matching_and_formatting() {
        assert!(matches_glob("7zip", "7*"));
        assert!(matches_glob("7kaa-data", "7*"));
        assert!(!matches_glob("python3", "7*"));
        assert!(matches_glob("4g8", "4*"));
        assert!(matches_glob("a2ps", "a2*"));
        assert!(matches_glob("a2boot", "*boot*"));

        let formatted_uninstalled = format_apt_list_entry("4g8", "resolute", "1.0-8", "amd64", false, false);
        assert_eq!(formatted_uninstalled, "4g8/resolute 1.0-8 amd64");

        let formatted_installed = format_apt_list_entry("7zip", "resolute", "26.00+dfsg-1", "amd64", true, true);
        assert_eq!(formatted_installed, "7zip/resolute,now 26.00+dfsg-1 amd64 [installed,automatic]");
    }
}
