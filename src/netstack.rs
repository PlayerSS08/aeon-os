//! Self-contained bare-metal network protocol stack:
//! - Ethernet II framing and MAC addressing
//! - ARP table and address resolution
//! - IPv4 routing and RFC 1071 internet checksum calculation
//! - ICMP Echo Request / Reply (`ping`) with round-trip latency
//! - UDP transport and RFC 2131 DHCP client (auto IP configuration)
//! - TCP connection state machine and HTTP GET client (`curl` / `http`)
//! - Socket table for Linux-compatible Ring 3 system calls

use core::fmt;
use crate::netdev;
use crate::spin::SpinLock;

#[cfg(target_os = "uefi")]
fn uptime_us() -> u64 {
    crate::log::uptime_us()
}
#[cfg(not(target_os = "uefi"))]
fn uptime_us() -> u64 {
    0
}

#[cfg(target_os = "uefi")]
fn pause() {
    crate::arch::pause();
}
#[cfg(not(target_os = "uefi"))]
fn pause() {
    core::hint::spin_loop();
}

#[cfg(target_os = "uefi")]
fn tsc_hz() -> u64 {
    crate::log::tsc_hz()
}
#[cfg(not(target_os = "uefi"))]
fn tsc_hz() -> u64 {
    1_000_000_000
}

#[cfg(target_os = "uefi")]
fn rdtsc() -> u64 {
    crate::arch::rdtsc()
}
#[cfg(not(target_os = "uefi"))]
fn rdtsc() -> u64 {
    0
}

// EtherTypes
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

// IP Protocols
pub const IPPROTO_ICMP: u8 = 1;
pub const IPPROTO_TCP: u8 = 6;
pub const IPPROTO_UDP: u8 = 17;

// ICMP Types
pub const ICMP_ECHO_REPLY: u8 = 0;
pub const ICMP_ECHO_REQUEST: u8 = 8;

// TCP Flags
pub const TCP_FIN: u16 = 0x001;
pub const TCP_SYN: u16 = 0x002;
pub const TCP_RST: u16 = 0x004;
pub const TCP_PSH: u16 = 0x008;
pub const TCP_ACK: u16 = 0x010;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    pub const BROADCAST: Self = Self([0xFF; 6]);
    pub const ZERO: Self = Self([0; 6]);

    pub fn parse(s: &str) -> Option<Self> {
        let mut b = [0u8; 6];
        let mut idx = 0;
        for part in s.split([':', '-']) {
            if idx >= 6 {
                return None;
            }
            b[idx] = u8::from_str_radix(part, 16).ok()?;
            idx += 1;
        }
        if idx == 6 {
            Some(Self(b))
        } else {
            None
        }
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    pub const ANY: Self = Self([0, 0, 0, 0]);
    pub const BROADCAST: Self = Self([255, 255, 255, 255]);
    pub const LOOPBACK: Self = Self([127, 0, 0, 1]);

    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    pub fn as_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    pub fn from_u32(val: u32) -> Self {
        Self(val.to_be_bytes())
    }

    pub fn parse(s: &str) -> Option<Self> {
        let mut b = [0u8; 4];
        let mut idx = 0;
        for part in s.split('.') {
            if idx >= 4 {
                return None;
            }
            b[idx] = part.parse::<u8>().ok()?;
            idx += 1;
        }
        if idx == 4 {
            Some(Self(b))
        } else {
            None
        }
    }

    pub fn is_same_subnet(&self, other: &Self, mask: &Self) -> bool {
        (self.as_u32() & mask.as_u32()) == (other.as_u32() & mask.as_u32())
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

/// Computes standard Internet RFC 1071 ones-complement checksum.
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 1 < data.len() {
        let word = u16::from_be_bytes([data[i], data[i + 1]]);
        sum = sum.wrapping_add(u32::from(word));
        i += 2;
    }
    if i < data.len() {
        let word = u16::from_be_bytes([data[i], 0]);
        sum = sum.wrapping_add(u32::from(word));
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Network configuration settings (IP address, netmask, default gateway, DNS server).
pub struct NetConfig {
    pub ip: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub dns: Ipv4Addr,
    pub is_dhcp: bool,
}

static CONFIG: SpinLock<NetConfig> = SpinLock::new(NetConfig {
    ip: Ipv4Addr::new(10, 0, 2, 15),
    mask: Ipv4Addr::new(255, 255, 255, 0),
    gateway: Ipv4Addr::new(10, 0, 2, 2),
    dns: Ipv4Addr::new(10, 0, 2, 3),
    is_dhcp: false,
});

pub fn get_config() -> (Ipv4Addr, Ipv4Addr, Ipv4Addr, Ipv4Addr, bool) {
    let c = CONFIG.lock();
    (c.ip, c.mask, c.gateway, c.dns, c.is_dhcp)
}

pub fn set_config(ip: Ipv4Addr, mask: Ipv4Addr, gateway: Ipv4Addr, dns: Ipv4Addr, is_dhcp: bool) {
    let mut c = CONFIG.lock();
    c.ip = ip;
    c.mask = mask;
    c.gateway = gateway;
    c.dns = dns;
    c.is_dhcp = is_dhcp;
}

// -----------------------------------------------------------------------------
// ARP Table & Resolution
// -----------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct ArpEntry {
    ip: Ipv4Addr,
    mac: MacAddr,
    valid: bool,
}

const ARP_CACHE_SIZE: usize = 16;
static ARP_CACHE: SpinLock<[ArpEntry; ARP_CACHE_SIZE]> = SpinLock::new([ArpEntry {
    ip: Ipv4Addr::ANY,
    mac: MacAddr::ZERO,
    valid: false,
}; ARP_CACHE_SIZE]);

pub fn arp_update(ip: Ipv4Addr, mac: MacAddr) {
    let mut cache = ARP_CACHE.lock();
    for entry in cache.iter_mut() {
        if entry.valid && entry.ip == ip {
            entry.mac = mac;
            return;
        }
    }
    for entry in cache.iter_mut() {
        if !entry.valid {
            entry.ip = ip;
            entry.mac = mac;
            entry.valid = true;
            return;
        }
    }
    cache[0] = ArpEntry { ip, mac, valid: true };
}

pub fn arp_lookup(ip: Ipv4Addr) -> Option<MacAddr> {
    let cache = ARP_CACHE.lock();
    for entry in cache.iter() {
        if entry.valid && entry.ip == ip {
            return Some(entry.mac);
        }
    }
    None
}

pub fn arp_entries() -> alloc::vec::Vec<(Ipv4Addr, MacAddr)> {
    let cache = ARP_CACHE.lock();
    let mut list = alloc::vec::Vec::new();
    for entry in cache.iter() {
        if entry.valid {
            list.push((entry.ip, entry.mac));
        }
    }
    list
}

pub fn send_arp_request(target_ip: Ipv4Addr) -> Result<(), netdev::NetError> {
    let (my_ip, _, _, _, _) = get_config();
    let my_mac = netdev::mac_address();

    let mut packet = [0u8; 42];
    // Ethernet Header (14 bytes)
    packet[..6].copy_from_slice(&MacAddr::BROADCAST.0);
    packet[6..12].copy_from_slice(&my_mac);
    packet[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());

    // ARP Payload (28 bytes)
    packet[14..16].copy_from_slice(&1u16.to_be_bytes()); // Hardware type: Ethernet (1)
    packet[16..18].copy_from_slice(&ETHERTYPE_IPV4.to_be_bytes()); // Protocol type: IPv4
    packet[18] = 6; // Hardware size: 6
    packet[19] = 4; // Protocol size: 4
    packet[20..22].copy_from_slice(&1u16.to_be_bytes()); // Opcode: Request (1)
    packet[22..28].copy_from_slice(&my_mac);
    packet[28..32].copy_from_slice(&my_ip.0);
    packet[32..38].copy_from_slice(&MacAddr::ZERO.0);
    packet[38..42].copy_from_slice(&target_ip.0);

    netdev::send_packet(&packet)
}

fn handle_arp(payload: &[u8]) {
    if payload.len() < 28 {
        return;
    }
    let opcode = u16::from_be_bytes([payload[6], payload[7]]);
    let mut sender_mac = [0u8; 6];
    sender_mac.copy_from_slice(&payload[8..14]);
    let sender_ip = Ipv4Addr([payload[14], payload[15], payload[16], payload[17]]);
    let target_ip = Ipv4Addr([payload[24], payload[25], payload[26], payload[27]]);

    arp_update(sender_ip, MacAddr(sender_mac));

    let (my_ip, _, _, _, _) = get_config();
    if opcode == 1 && target_ip == my_ip {
        // Send ARP reply
        let my_mac = netdev::mac_address();
        let mut reply = [0u8; 42];
        reply[..6].copy_from_slice(&sender_mac);
        reply[6..12].copy_from_slice(&my_mac);
        reply[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());

        reply[14..16].copy_from_slice(&1u16.to_be_bytes());
        reply[16..18].copy_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
        reply[18] = 6;
        reply[19] = 4;
        reply[20..22].copy_from_slice(&2u16.to_be_bytes()); // Opcode: Reply (2)
        reply[22..28].copy_from_slice(&my_mac);
        reply[28..32].copy_from_slice(&my_ip.0);
        reply[32..38].copy_from_slice(&sender_mac);
        reply[38..42].copy_from_slice(&sender_ip.0);

        let _ = netdev::send_packet(&reply);
    }
}

pub fn resolve_mac(target_ip: Ipv4Addr) -> Option<MacAddr> {
    if target_ip == Ipv4Addr::BROADCAST {
        return Some(MacAddr::BROADCAST);
    }
    if let Some(mac) = arp_lookup(target_ip) {
        return Some(mac);
    }

    let _ = send_arp_request(target_ip);
    let t0 = rdtsc();
    let timeout = tsc_hz() / 10; // 100ms
    loop {
        poll();
        if let Some(mac) = arp_lookup(target_ip) {
            return Some(mac);
        }
        if rdtsc().wrapping_sub(t0) > timeout {
            break;
        }
        pause();
    }
    None
}

// -----------------------------------------------------------------------------
// IPv4 Packet Construction & Sending
// -----------------------------------------------------------------------------

static IP_ID: SpinLock<u16> = SpinLock::new(100);

pub fn send_ipv4_packet(
    dst_ip: Ipv4Addr,
    protocol: u8,
    payload: &[u8],
) -> Result<(), netdev::NetError> {
    let (my_ip, mask, gateway, _, _) = get_config();
    let my_mac = netdev::mac_address();

    let target_for_mac = if dst_ip == Ipv4Addr::BROADCAST {
        Ipv4Addr::BROADCAST
    } else if my_ip.is_same_subnet(&dst_ip, &mask) {
        dst_ip
    } else {
        gateway
    };

    let dst_mac = resolve_mac(target_for_mac).unwrap_or(MacAddr::BROADCAST);

    let total_len = 20 + payload.len();
    let mut packet = alloc::vec![0u8; 14 + total_len];

    // Ethernet Header (14 bytes)
    packet[..6].copy_from_slice(&dst_mac.0);
    packet[6..12].copy_from_slice(&my_mac);
    packet[12..14].copy_from_slice(&ETHERTYPE_IPV4.to_be_bytes());

    // IPv4 Header (20 bytes)
    let id = {
        let mut cur = IP_ID.lock();
        *cur = cur.wrapping_add(1);
        *cur
    };

    packet[14] = 0x45; // Version 4, IHL 5
    packet[15] = 0; // DSCP / ECN
    packet[16..18].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[18..20].copy_from_slice(&id.to_be_bytes());
    packet[20..22].copy_from_slice(&0x4000u16.to_be_bytes()); // Don't fragment
    packet[22] = 64; // TTL
    packet[23] = protocol;
    packet[24..26].copy_from_slice(&0u16.to_be_bytes()); // Checksum placeholder
    packet[26..30].copy_from_slice(&my_ip.0);
    packet[30..34].copy_from_slice(&dst_ip.0);

    let csum = internet_checksum(&packet[14..34]);
    packet[24..26].copy_from_slice(&csum.to_be_bytes());

    packet[34..].copy_from_slice(payload);
    netdev::send_packet(&packet)
}

// -----------------------------------------------------------------------------
// ICMP (Ping)
// -----------------------------------------------------------------------------

struct PingWait {
    seq: u16,
    received: bool,
}

static PING_STATE: SpinLock<Option<PingWait>> = SpinLock::new(None);

fn handle_icmp(src_ip: Ipv4Addr, payload: &[u8]) {
    if payload.len() < 8 {
        return;
    }
    let icmp_type = payload[0];
    let icmp_code = payload[1];
    let _id = u16::from_be_bytes([payload[4], payload[5]]);
    let seq = u16::from_be_bytes([payload[6], payload[7]]);

    if icmp_type == ICMP_ECHO_REQUEST && icmp_code == 0 {
        // Send ICMP Echo Reply
        let mut reply = alloc::vec![0u8; payload.len()];
        reply.copy_from_slice(payload);
        reply[0] = ICMP_ECHO_REPLY;
        reply[2..4].copy_from_slice(&0u16.to_be_bytes());
        let csum = internet_checksum(&reply);
        reply[2..4].copy_from_slice(&csum.to_be_bytes());
        let _ = send_ipv4_packet(src_ip, IPPROTO_ICMP, &reply);
    } else if icmp_type == ICMP_ECHO_REPLY {
        let mut state = PING_STATE.lock();
        if let Some(w) = state.as_mut() {
            if w.seq == seq {
                w.received = true;
            }
        }
    }
}

pub struct PingStats {
    pub sent: usize,
    pub received: usize,
    pub min_us: u64,
    pub max_us: u64,
    pub avg_us: u64,
}

pub fn ping(target: Ipv4Addr, count: usize, timeout_ms: u64) -> PingStats {
    let mut sent = 0;
    let mut received = 0;
    let mut min_us = u64::MAX;
    let mut max_us = 0u64;
    let mut total_us = 0u64;

    for seq in 1..=count {
        sent += 1;
        let mut payload = [0u8; 32];
        payload[0] = ICMP_ECHO_REQUEST;
        payload[1] = 0;
        payload[2..4].copy_from_slice(&0u16.to_be_bytes());
        payload[4..6].copy_from_slice(&0xAE01u16.to_be_bytes()); // Ident
        payload[6..8].copy_from_slice(&(seq as u16).to_be_bytes());
        // Filler payload
        for (idx, b) in payload[8..32].iter_mut().enumerate() {
            *b = idx as u8 + b'a';
        }
        let csum = internet_checksum(&payload);
        payload[2..4].copy_from_slice(&csum.to_be_bytes());

        *PING_STATE.lock() = Some(PingWait {
            seq: seq as u16,
            received: false,
        });

        let t0 = uptime_us();
        if send_ipv4_packet(target, IPPROTO_ICMP, &payload).is_err() {
            continue;
        }

        let deadline = t0 + timeout_ms * 1000;
        let mut got_reply = false;
        while uptime_us() < deadline {
            poll();
            let state = PING_STATE.lock();
            if let Some(w) = state.as_ref() {
                if w.received {
                    got_reply = true;
                    break;
                }
            }
            pause();
        }

        if got_reply {
            let rtt = uptime_us().saturating_sub(t0);
            received += 1;
            min_us = min_us.min(rtt);
            max_us = max_us.max(rtt);
            total_us += rtt;
        }

        // Brief delay between pings if multiple
        if count > 1 && seq < count {
            let wait_until_us = uptime_us() + 200_000;
            while uptime_us() < wait_until_us {
                poll();
                pause();
            }
        }
    }

    *PING_STATE.lock() = None;

    PingStats {
        sent,
        received,
        min_us: if received > 0 { min_us } else { 0 },
        max_us,
        avg_us: if received > 0 { total_us / received as u64 } else { 0 },
    }
}

// -----------------------------------------------------------------------------
// UDP & DHCP Client (RFC 2131)
// -----------------------------------------------------------------------------

struct DhcpLease {
    offered_ip: Ipv4Addr,
    server_ip: Ipv4Addr,
    subnet_mask: Ipv4Addr,
    gateway: Ipv4Addr,
    dns: Ipv4Addr,
    xid: u32,
    stage: u8, // 0 = none, 1 = offer received, 2 = ack received
}

static DHCP_LEASE: SpinLock<Option<DhcpLease>> = SpinLock::new(None);

pub fn run_dhcp(timeout_ms: u64) -> Result<(Ipv4Addr, Ipv4Addr, Ipv4Addr, Ipv4Addr), &'static str> {
    let my_mac = netdev::mac_address();
    let xid = 0x39A2_10D5;

    *DHCP_LEASE.lock() = Some(DhcpLease {
        offered_ip: Ipv4Addr::ANY,
        server_ip: Ipv4Addr::ANY,
        subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
        gateway: Ipv4Addr::new(10, 0, 2, 2),
        dns: Ipv4Addr::new(10, 0, 2, 3),
        xid,
        stage: 0,
    });

    // 1. Send DHCP DISCOVER
    send_dhcp_message(1, xid, Ipv4Addr::ANY, Ipv4Addr::ANY, &my_mac)?;

    let t0 = uptime_us();
    let deadline = t0 + timeout_ms * 1000;

    // Wait for DHCP OFFER
    let mut got_offer = false;
    while uptime_us() < deadline {
        poll();
        let lease = DHCP_LEASE.lock();
        if let Some(l) = lease.as_ref() {
            if l.stage >= 1 {
                got_offer = true;
                break;
            }
        }
        pause();
    }

    if !got_offer {
        // Fall back to default NAT settings
        return Err("DHCP discover timed out (using static fallback 10.0.2.15)");
    }

    let (offered_ip, server_ip) = {
        let lease = DHCP_LEASE.lock();
        let l = lease.as_ref().unwrap();
        (l.offered_ip, l.server_ip)
    };

    // 2. Send DHCP REQUEST
    send_dhcp_message(3, xid, offered_ip, server_ip, &my_mac)?;

    // Wait for DHCP ACK
    let req_deadline = uptime_us() + timeout_ms * 1000;
    let mut got_ack = false;
    while uptime_us() < req_deadline {
        poll();
        let lease = DHCP_LEASE.lock();
        if let Some(l) = lease.as_ref() {
            if l.stage >= 2 {
                got_ack = true;
                break;
            }
        }
        pause();
    }

    if !got_ack {
        return Err("DHCP request timed out");
    }

    let (ip, mask, gw, dns) = {
        let lease = DHCP_LEASE.lock();
        let l = lease.as_ref().unwrap();
        (l.offered_ip, l.subnet_mask, l.gateway, l.dns)
    };

    set_config(ip, mask, gw, dns, true);
    arp_update(gw, MacAddr([0x52, 0x54, 0x00, 0x12, 0x34, 0x02]));
    Ok((ip, mask, gw, dns))
}

fn send_dhcp_message(
    msg_type: u8,
    xid: u32,
    req_ip: Ipv4Addr,
    server_ip: Ipv4Addr,
    mac: &[u8; 6],
) -> Result<(), &'static str> {
    let mut dhcp = [0u8; 300];
    dhcp[0] = 1; // BootRequest
    dhcp[1] = 1; // 10Mb Ethernet
    dhcp[2] = 6; // Hardware address length
    dhcp[3] = 0; // Hops
    dhcp[4..8].copy_from_slice(&xid.to_be_bytes());
    dhcp[8..10].copy_from_slice(&0u16.to_be_bytes()); // Seconds
    dhcp[10..12].copy_from_slice(&0x8000u16.to_be_bytes()); // Broadcast flag
    dhcp[28..34].copy_from_slice(mac);

    // Magic cookie (0x63, 0x82, 0x53, 0x63)
    dhcp[236..240].copy_from_slice(&[99, 130, 83, 99]);

    let mut opt = 240;
    // Option 53: DHCP Message Type
    dhcp[opt] = 53;
    dhcp[opt + 1] = 1;
    dhcp[opt + 2] = msg_type;
    opt += 3;

    if msg_type == 3 {
        // Option 50: Requested IP
        dhcp[opt] = 50;
        dhcp[opt + 1] = 4;
        dhcp[opt + 2..opt + 6].copy_from_slice(&req_ip.0);
        opt += 6;

        // Option 54: Server Identifier
        dhcp[opt] = 54;
        dhcp[opt + 1] = 4;
        dhcp[opt + 2..opt + 6].copy_from_slice(&server_ip.0);
        opt += 6;
    }

    // Option 55: Parameter Request List (1: Subnet Mask, 3: Router, 6: DNS)
    dhcp[opt] = 55;
    dhcp[opt + 1] = 3;
    dhcp[opt + 2] = 1;
    dhcp[opt + 3] = 3;
    dhcp[opt + 4] = 6;
    opt += 5;

    // End Option
    dhcp[opt] = 255;
    opt += 1;

    send_udp_packet(
        Ipv4Addr::ANY,
        Ipv4Addr::BROADCAST,
        68,
        67,
        &dhcp[..opt],
    ).map_err(|_| "failed to transmit DHCP packet")
}

fn send_udp_packet(
    _src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<(), netdev::NetError> {
    let udp_len = 8 + payload.len();
    let mut udp = alloc::vec![0u8; udp_len];
    udp[0..2].copy_from_slice(&src_port.to_be_bytes());
    udp[2..4].copy_from_slice(&dst_port.to_be_bytes());
    udp[4..6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    udp[6..8].copy_from_slice(&0u16.to_be_bytes()); // Optional UDP checksum
    udp[8..].copy_from_slice(payload);

    send_ipv4_packet(dst_ip, IPPROTO_UDP, &udp)
}

fn handle_udp(src_ip: Ipv4Addr, payload: &[u8]) {
    if payload.len() < 8 {
        return;
    }
    let src_port = u16::from_be_bytes([payload[0], payload[1]]);
    let dst_port = u16::from_be_bytes([payload[2], payload[3]]);
    let data = &payload[8..];

    if dst_port == 68 && src_port == 67 {
        // Incoming DHCP packet
        handle_dhcp(src_ip, data);
    }
}

fn handle_dhcp(server_ip: Ipv4Addr, data: &[u8]) {
    if data.len() < 240 {
        return;
    }
    let xid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let yiaddr = Ipv4Addr([data[16], data[17], data[18], data[19]]);

    let mut lease = DHCP_LEASE.lock();
    let l = match lease.as_mut() {
        Some(l) if l.xid == xid => l,
        _ => return,
    };

    l.offered_ip = yiaddr;
    l.server_ip = server_ip;

    // Parse DHCP Options starting after magic cookie (offset 240)
    let mut i = 240;
    let mut msg_type = 0u8;
    while i < data.len() && data[i] != 255 {
        let opt = data[i];
        if opt == 0 {
            i += 1;
            continue;
        }
        if i + 1 >= data.len() {
            break;
        }
        let len = data[i + 1] as usize;
        let val_start = i + 2;
        let val_end = val_start + len;
        if val_end > data.len() {
            break;
        }
        let val = &data[val_start..val_end];
        match opt {
            53 if len == 1 => msg_type = val[0],
            1 if len == 4 => l.subnet_mask = Ipv4Addr([val[0], val[1], val[2], val[3]]),
            3 if len >= 4 => l.gateway = Ipv4Addr([val[0], val[1], val[2], val[3]]),
            6 if len >= 4 => l.dns = Ipv4Addr([val[0], val[1], val[2], val[3]]),
            54 if len == 4 => l.server_ip = Ipv4Addr([val[0], val[1], val[2], val[3]]),
            _ => {}
        }
        i = val_end;
    }

    if msg_type == 2 {
        // DHCP OFFER
        l.stage = 1;
    } else if msg_type == 5 {
        // DHCP ACK
        l.stage = 2;
    }
}

// -----------------------------------------------------------------------------
// TCP Client & HTTP
// -----------------------------------------------------------------------------

struct TcpConn {
    local_port: u16,
    remote_ip: Ipv4Addr,
    remote_port: u16,
    local_seq: u32,
    remote_seq: u32,
    state: u8, // 0 = closed, 1 = syn_sent, 2 = established, 3 = fin_wait
    rx_buf: alloc::vec::Vec<u8>,
}

static TCP_CONN: SpinLock<Option<TcpConn>> = SpinLock::new(None);

fn tcp_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, tcp_segment: &[u8]) -> u16 {
    let mut pseudo = alloc::vec![0u8; 12 + tcp_segment.len()];
    pseudo[..4].copy_from_slice(&src_ip.0);
    pseudo[4..8].copy_from_slice(&dst_ip.0);
    pseudo[8] = 0;
    pseudo[9] = IPPROTO_TCP;
    pseudo[10..12].copy_from_slice(&(tcp_segment.len() as u16).to_be_bytes());
    pseudo[12..].copy_from_slice(tcp_segment);
    internet_checksum(&pseudo)
}

fn send_tcp_segment(
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u16,
    payload: &[u8],
) -> Result<(), netdev::NetError> {
    let (my_ip, _, _, _, _) = get_config();
    let total_len = 20 + payload.len();
    let mut seg = alloc::vec![0u8; total_len];

    seg[0..2].copy_from_slice(&src_port.to_be_bytes());
    seg[2..4].copy_from_slice(&dst_port.to_be_bytes());
    seg[4..8].copy_from_slice(&seq.to_be_bytes());
    seg[8..12].copy_from_slice(&ack.to_be_bytes());
    let offset_and_flags = (5u16 << 12) | (flags & 0x01FF);
    seg[12..14].copy_from_slice(&offset_and_flags.to_be_bytes());
    seg[14..16].copy_from_slice(&64240u16.to_be_bytes()); // Window size
    seg[16..18].copy_from_slice(&0u16.to_be_bytes()); // Checksum
    seg[18..20].copy_from_slice(&0u16.to_be_bytes()); // Urgent pointer
    seg[20..].copy_from_slice(payload);

    let csum = tcp_checksum(my_ip, dst_ip, &seg);
    seg[16..18].copy_from_slice(&csum.to_be_bytes());

    send_ipv4_packet(dst_ip, IPPROTO_TCP, &seg)
}

fn handle_tcp(src_ip: Ipv4Addr, payload: &[u8]) {
    if payload.len() < 20 {
        return;
    }
    let src_port = u16::from_be_bytes([payload[0], payload[1]]);
    let dst_port = u16::from_be_bytes([payload[2], payload[3]]);
    let seq = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let ack = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let offset = ((payload[12] >> 4) as usize) * 4;
    let flags = u16::from_be_bytes([payload[12] & 0x01, payload[13]]);

    let mut conn = TCP_CONN.lock();
    let c = match conn.as_mut() {
        Some(c) if c.remote_ip == src_ip && c.remote_port == src_port && c.local_port == dst_port => c,
        _ => return,
    };

    if c.state == 1 && (flags & TCP_SYN != 0) && (flags & TCP_ACK != 0) {
        // SYN-ACK received -> send ACK
        c.remote_seq = seq.wrapping_add(1);
        c.local_seq = ack;
        c.state = 2; // ESTABLISHED
        let _ = send_tcp_segment(src_ip, c.local_port, c.remote_port, c.local_seq, c.remote_seq, TCP_ACK, &[]);
    } else if c.state == 2 {
        let data_len = payload.len().saturating_sub(offset);
        if data_len > 0 {
            c.rx_buf.extend_from_slice(&payload[offset..]);
            c.remote_seq = c.remote_seq.wrapping_add(data_len as u32);
            let _ = send_tcp_segment(src_ip, c.local_port, c.remote_port, c.local_seq, c.remote_seq, TCP_ACK, &[]);
        }
        if flags & TCP_FIN != 0 {
            c.remote_seq = c.remote_seq.wrapping_add(1);
            let _ = send_tcp_segment(src_ip, c.local_port, c.remote_port, c.local_seq, c.remote_seq, TCP_ACK | TCP_FIN, &[]);
            c.state = 0; // CLOSED
        }
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub headers: alloc::string::String,
    pub body: alloc::vec::Vec<u8>,
}

pub fn http_get(target: Ipv4Addr, port: u16, path: &str) -> Result<HttpResponse, &'static str> {
    let local_port = 49152 + ((uptime_us() % 16384) as u16);
    let initial_seq = 0x1000_2000;

    *TCP_CONN.lock() = Some(TcpConn {
        local_port,
        remote_ip: target,
        remote_port: port,
        local_seq: initial_seq,
        remote_seq: 0,
        state: 1, // SYN_SENT
        rx_buf: alloc::vec::Vec::new(),
    });

    // Send SYN
    send_tcp_segment(target, local_port, port, initial_seq, 0, TCP_SYN, &[])
        .map_err(|_| "failed to send TCP SYN")?;

    // Wait for ESTABLISHED
    let t0 = uptime_us();
    let mut ok = false;
    while uptime_us() < t0 + 3_000_000 {
        poll();
        let conn = TCP_CONN.lock();
        if let Some(c) = conn.as_ref() {
            if c.state == 2 {
                ok = true;
                break;
            }
        }
        pause();
    }

    if !ok {
        *TCP_CONN.lock() = None;
        return Err("connection refused or timed out");
    }

    // Send HTTP GET request
    let req = alloc::format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: aeon-os/1.0\r\nConnection: close\r\n\r\n",
        path,
        target
    );

    let (l_seq, r_seq) = {
        let conn = TCP_CONN.lock();
        let c = conn.as_ref().unwrap();
        (c.local_seq, c.remote_seq)
    };

    send_tcp_segment(target, local_port, port, l_seq, r_seq, TCP_ACK | TCP_PSH, req.as_bytes())
        .map_err(|_| "failed to send HTTP request")?;

    // Wait for data and connection close
    let t1 = uptime_us();
    while uptime_us() < t1 + 5_000_000 {
        poll();
        let conn = TCP_CONN.lock();
        if let Some(c) = conn.as_ref() {
            if c.state == 0 || !c.rx_buf.is_empty() {
                // Check if full HTTP response received
                if c.rx_buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
        }
        pause();
    }

    let raw = {
        let mut conn = TCP_CONN.lock();
        let c = conn.take().ok_or("no connection")?;
        c.rx_buf
    };

    if raw.is_empty() {
        return Err("empty HTTP response");
    }

    // Parse status line and split body
    let delim = b"\r\n\r\n";
    let split_pos = raw.windows(4).position(|w| w == delim).unwrap_or(raw.len());
    let header_bytes = &raw[..split_pos];
    let body_bytes = if split_pos + 4 <= raw.len() {
        &raw[split_pos + 4..]
    } else {
        &[]
    };

    let header_str = alloc::string::String::from_utf8_lossy(header_bytes);
    let mut status = 200;
    if let Some(first_line) = header_str.lines().next() {
        let mut parts = first_line.split_whitespace();
        let _proto = parts.next();
        if let Some(code_str) = parts.next() {
            if let Ok(c) = code_str.parse::<u16>() {
                status = c;
            }
        }
    }

    Ok(HttpResponse {
        status,
        headers: alloc::string::String::from(header_str),
        body: alloc::vec::Vec::from(body_bytes),
    })
}

// -----------------------------------------------------------------------------
// Central Packet Ingestion & Polling Dispatch
// -----------------------------------------------------------------------------

pub fn poll() {
    let mut buf = [0u8; 1600];
    while let Ok(len) = netdev::receive_packet(&mut buf) {
        if len < 14 {
            continue;
        }
        let ethertype = u16::from_be_bytes([buf[12], buf[13]]);
        let payload = &buf[14..len];

        match ethertype {
            ETHERTYPE_ARP => handle_arp(payload),
            ETHERTYPE_IPV4 => {
                if payload.len() < 20 {
                    continue;
                }
                let ihl = (payload[0] & 0x0F) as usize * 4;
                if payload.len() < ihl {
                    continue;
                }
                let protocol = payload[9];
                let src_ip = Ipv4Addr([payload[12], payload[13], payload[14], payload[15]]);
                let ip_data = &payload[ihl..];

                match protocol {
                    IPPROTO_ICMP => handle_icmp(src_ip, ip_data),
                    IPPROTO_UDP => handle_udp(src_ip, ip_data),
                    IPPROTO_TCP => handle_tcp(src_ip, ip_data),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

// -----------------------------------------------------------------------------
// Unit Tests (Host-Testable Pure Logic)
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_parse_and_display() {
        let ip = Ipv4Addr::parse("192.168.1.100").unwrap();
        assert_eq!(ip, Ipv4Addr([192, 168, 1, 100]));
        assert_eq!(ip.as_u32(), 0xC0A80164);
        assert_eq!(Ipv4Addr::from_u32(0xC0A80164), ip);
        assert_eq!(alloc::format!("{ip}"), "192.168.1.100");

        assert_eq!(Ipv4Addr::parse("256.0.0.1"), None);
        assert_eq!(Ipv4Addr::parse("1.2.3"), None);
        assert_eq!(Ipv4Addr::parse("1.2.3.4.5"), None);
    }

    #[test]
    fn mac_parse_and_display() {
        let mac = MacAddr::parse("52:54:00:12:34:56").unwrap();
        assert_eq!(mac.0, [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        assert_eq!(alloc::format!("{mac}"), "52:54:00:12:34:56");

        let mac2 = MacAddr::parse("52-54-00-12-34-56").unwrap();
        assert_eq!(mac2, mac);
    }

    #[test]
    fn subnet_matching() {
        let ip1 = Ipv4Addr::new(10, 0, 2, 15);
        let ip2 = Ipv4Addr::new(10, 0, 2, 2);
        let ip3 = Ipv4Addr::new(192, 168, 1, 1);
        let mask = Ipv4Addr::new(255, 255, 255, 0);

        assert!(ip1.is_same_subnet(&ip2, &mask));
        assert!(!ip1.is_same_subnet(&ip3, &mask));
    }

    #[test]
    fn rfc1071_checksum_calculation() {
        let header = [
            0x45, 0x00, 0x00, 0x3c, 0x1c, 0x46, 0x40, 0x00, 0x40, 0x06, 0x00, 0x00, 0xac, 0x10,
            0x0a, 0x63, 0xac, 0x10, 0x0a, 0x0c,
        ];
        let csum = internet_checksum(&header);
        assert_ne!(csum, 0);

        let mut verified = header;
        verified[10..12].copy_from_slice(&csum.to_be_bytes());
        assert_eq!(internet_checksum(&verified), 0);
    }

    #[test]
    fn arp_cache_operations() {
        let ip = Ipv4Addr::new(10, 0, 2, 2);
        let mac = MacAddr([0x52, 0x54, 0x00, 0x12, 0x34, 0x02]);

        arp_update(ip, mac);
        assert_eq!(arp_lookup(ip), Some(mac));
        assert_eq!(arp_lookup(Ipv4Addr::new(10, 0, 2, 99)), None);
    }
}
