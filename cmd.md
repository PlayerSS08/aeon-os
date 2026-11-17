# Aeon OS Command Reference Manual (`cmd.md`)

This document provides the definitive command catalog, operational syntax, hardware requirements, and practical verification examples for Aeon OS.

---

## 1. Network Connectivity & Access

Aeon OS features a bare-metal TCP/IP protocol stack supporting ARP, IPv4, UDP, DHCP, ICMP, and TCP.

### Ethernet Wired Connection (Fully Supported)
The bare-metal Intel e1000 Gigabit NIC (`82540EM` / `82545EM` / `82574L`) and VirtIO Network devices are natively supported.

- **Automated DHCP Lease Acquisition:**
  ```text
  connect
  ```
  *(or `dhcp` / `connect ethernet`)*
  - **Action:** Broadcasts DHCP Discover, negotiates lease with gateway router, dynamically configures IPv4 address, subnet mask, default gateway, and DNS server.
  - **Output Example:**
    ```text
    initiating DHCP lease request on eth0...
    DHCP lease acquired successfully!
      IP Address    : 10.0.2.15
      Subnet Mask   : 255.255.255.0
      Gateway Router: 10.0.2.2
      DNS Server    : 10.196.213.22
    ```

- **Manual Static IP Configuration:**
  ```text
  ifconfig eth0 <ip> netmask <mask_ip>
  ```
  - **Example:** `ifconfig eth0 192.168.1.50 netmask 255.255.255.0`

- **Test Gateway / Internet Access:**
  ```text
  ping gateway
  ping 8.8.8.8 -c 4
  ```
  - **Action:** Transmits ICMP Echo Requests and calculates round-trip time (RTT) statistics.

- **Network Interface Statistics:**
  ```text
  ifconfig
  ip a
  netstat
  ```

---

### Wi-Fi Wireless Connection (honest status)

- **Commands:** `wifi status`, `wifi scan`, `connect wifi <ssid> <password>`.
- **What happens today:** they report the Wi-Fi chip found on the PCI bus (on the HP OMEN 16: Realtek RTL8852BE, `10EC:B520`) and refuse to scan or join, because **no driver for that chip exists yet**.
- **What is built and tested:** everything above the radio. WPA2-Personal key derivation (PBKDF2-SHA1), the 4-way EAPOL handshake, AES-CCMP encryption, 802.11 scan/auth/association frames and the station state machine (`src/wpa.rs`, `src/ieee80211.rs`, `src/wifi.rs`). They are tested against RFC vectors, an independent Python implementation and a simulated access point, **not** against real Wi-Fi hardware.
- **What is missing:** the chip driver. The RTL8852BE needs Realtek's closed firmware, MAC/baseband setup tables and DMA rings; Linux's `rtw89` driver is large and its firmware has had version and crash problems (the OMEN's chip is the `-VT` variant, `rtw8852bt` in Linux). It can only be developed with the laptop in the loop.
- **Use instead:** USB tethering from a phone (next section), the wired port (Realtek RTL8168, driver included, *untested on hardware*: run `dhcp`) or a USB Ethernet adapter.

---

### Internet through a phone (USB tethering)

This is the route to the internet on the OMEN today, without a Wi-Fi chip driver: the phone does the Wi-Fi or mobile data, Aeon talks to the phone over USB.

1. Plug the phone into a USB port of the laptop (before or after boot).
2. On the phone: *Settings, Network, Hotspot and tethering, **USB tethering*** on (Android). It shows up as an RNDIS network function. CDC-ECM adapters (most USB Ethernet dongles, some phones) work too.
3. In Aeon run `dhcp`. It looks for a newly plugged USB network function, brings it up, and asks the phone for an address. Then `curl http://...` and `apt update` work.
4. Boot log lines to look for: `USB network function found`, `USB network ready ... MAC ...`. `usb` shows the ports.

Tested: DHCP and an HTTP download through QEMU's RNDIS device (`python3 tests/boot_test.py -k usbnet`). **Not yet tested with a real phone**: send the `log` output if it does not come up. iPhone tethering is a different protocol (not supported).

### Touchpad (I2C)

The OMEN's touchpad is not USB: it is an ELAN device on an Intel serial-bus (I2C) controller. Aeon has a driver stack for it (`src/dwi2c.rs`, `src/i2chid.rs`, `src/hidparse.rs`, `src/touchpad.rs`, `src/touchpad_hw.rs`): a DesignWare I2C master, the HID-over-I2C protocol, a HID report-descriptor parser and a Precision Touchpad decoder.

- One finger moves the pointer, a quick light tap is a left click, the pad's own button clicks (right button with two fingers down), two fingers moving up/down scroll the log.
- Without ACPI the controller and address are searched for: Intel serial-bus controllers on PCI (the OMEN's `8086:7A4C` first), addresses `0x15 0x2C 0x10 0x20 0x38 0x2A`, descriptor registers `0x0001 0x0020`. Input is polled (the touchpad's interrupt GPIO is not driven).
- `touchpad` prints what the probe did, step by step. `touchpad scan` lists every I2C address that answers on each controller. `touchpad dump` prints the device's report descriptor in hex. `touchpad rescan` probes again.
- **Tested on the host** against a fake controller and a fake touchpad (protocol, FIFO flow control, NACK handling, finger tracking, taps, scrolling). **Not tested on the real laptop**: if the pointer does not move, run `touchpad` and `touchpad scan` and send the text.

---

## 2. Memory Access & Inspection

Aeon OS provides kernel-level physical and virtual memory inspection tools.

- **`mem` (or `free`):**
  - Displays total physical RAM detected via UEFI, usable memory, page frame allocation counts, and kernel heap consumption.
  - **Example:**
    ```text
    aeon> mem
    ram        24205 MB total, 23552 MB usable at boot, top 24576 MB
    frames     5824901 free of 6031360 (22753 MB free, largest run 21450 MB)
    heap       84 KiB used of 1024 KiB, 3 free holes
    ```

- **`memmap`:**
  - Dumps the UEFI physical address descriptors, physical start/end boundaries, page count, and region classification (`ConventionalMemory`, `ACPIReclaimMemory`, `BootServicesData`, `RuntimeServicesCode`).

- **`alloc <pages>`:**
  - Allocates contiguous physical memory frames, performs write/read pattern verification to test RAM integrity, and safely releases them.

---

## 3. Storage Subsystem & Drive Save Access

Aeon reads FAT16/FAT32 disks and writes to **one** place: a FAT32 USB stick whose volume label is `AEON`. Every other volume (including the laptop's internal NVMe with Windows) is never written. `EFI/` on the stick is protected too.

### Permanent Drive Saving (Disk Persistence)
Aeon distinguishes the temporary RAM filesystem (`/`, gone at power off) from the AEON USB stick (`/usb`, permanent). Create the stick with `mkfs.fat -F 32 -n AEON` or format FAT32 and set the label `AEON`.

- **Save / Create a File on Disk:**
  ```text
  write /usb/<filename> <content>
  ```
  - **Example:**
    ```text
    write /usb/notes.txt Configuration saved successfully.
    ```
  - **Behavior:** Writes the file to the stick's FAT32 volume (`usbfs`). Run `sync` before unplugging.

- **Overwrite Existing File on Disk:**
  ```text
  write -f /usb/<filename> <new_content>
  ```

- **Append Data to an Existing File:**
  ```text
  append /usb/<filename> <additional_line>
  ```
  - **Example:**
    ```text
    append /usb/notes.txt Appending line 2 to persistent storage.
    ```

- **Read Saved File from Disk:**
  ```text
  cat /usb/notes.txt
  ```

- **Inspect Disk Filesystem:**
  ```text
  ls /usb
  df
  ```
  - Displays total storage capacity, allocated clusters, and available free disk sectors.

- **Low-Level Direct Block Storage Operations:**
  - `disks`: Lists recognized storage devices (SATA AHCI ports, virtio disks, the USB stick).
  - `dls`: Traverses partitions and raw directories.
  - `dread <lba> [offset]`: Direct hex inspection of any raw 512-byte disk sector.
  - `storage`: Hardware diagnostics of AHCI SATA controllers, port speeds, and DMA command slots.

---

## 4. Packages & Internet (`apt`, `curl`)

Aeon's `apt` uses Debian's verbs but is **not** Debian's apt: it does not read `.deb` files and cannot run Linux programs. Packages are small archives of files (data, documents, scripts, Aeon programs) from a **signed repository**.

### How trust works
The kernel contains the repository's public Ed25519 key (`src/pkgkey.rs`). `apt update` downloads `index` and `index.sig` and refuses the list unless the signature verifies and its serial is not older than one already seen (rollback protection, remembered in RAM and on the stick). The index lists each package's size and SHA-256, so a package is installed only if its bytes match the signed index. Plain `http://` is therefore safe as a transport; `https://` is not supported yet (Aeon has no TLS).

### Typical session
```text
dhcp                                   get an address on the cable
apt source http://192.168.1.20:8080/   the PC that runs: python tools/aeon_repo.py serve repo
apt update                             "3 package(s) available"
apt list                               what the repository has
apt install greeter                    installs greet-data first, checks every hash
cat /usr/share/greet/en.txt
apt remove greeter ; apt autoremove
```
With the AEON stick plugged in, every installed package and the index are copied to `aeonpkg/` on the stick. After a reboot (even with no network) `apt restore` verifies the saved index and reinstalls them.

### Commands
| Command | Meaning |
|:---|:---|
| `apt source [url]` | show or set the repository address |
| `apt update` | fetch and verify the package list |
| `apt list [--installed]`, `apt search <text>`, `apt show <pkg>`, `apt files <pkg>` | queries |
| `apt install <pkg>...` | install with dependencies; refuses on bad signature, size, hash, unsafe paths or file conflicts; rolls back if the filesystem is full |
| `apt remove <pkg>...`, `apt autoremove` | remove; refuses while another package needs it |
| `apt restore` | reinstall what is saved on the stick |

### Making your own repository
```text
python tools/aeon_repo.py keygen            one time: key in .secrets/ (never commit), public key into src/pkgkey.rs, rebuild Aeon
python tools/aeon_repo.py build repo-src repo
python tools/aeon_repo.py serve repo --port 8080
```
A package source is a folder with `PKGINFO` (Package, Version, Depends, Description) and a `files/` tree whose paths are relative to `/`.

### Downloading anything else
`curl <http://host[:port]/path> [-o file]` resolves names with DNS, follows redirects, handles chunked replies and keeps up to 12 MiB. `-o /usb/file` saves to the stick. `sha256sum <file>` checks a download. Real network traffic has been tested against QEMU's network and the public internet (DNS plus a page fetch), over the Intel e1000 emulation; the Realtek wired driver has not run on real hardware yet.

---

## 5. Complete Shell Command Catalog

### Packaging & Internet Resources
| Command | Usage | Description |
|:---|:---|:---|
| `apt` | `apt <cmd> [pkg]` | Signed package manager: source, update, list, search, show, files, install, remove, autoremove, restore. |
| `apt-get`, `apt-cache` | same as `apt` | Aliases (`apt-cache` lists). |
| `curl` | `curl <url> [-o file]` | HTTP download with DNS and redirects. |
| `sha256sum` | `sha256sum <file>...` | SHA-256 of files, to verify downloads. |

### System & Architecture
| Command | Usage | Description |
|:---|:---|:---|
| `help` | `help [command]` | Display comprehensive command catalog or help for specific command. |
| `version` | `version` | Display Aeon OS release version, Git commit hash, and build profile. |
| `uname` | `uname` | Operating system identity and target architecture (`x86_64 UEFI`). |
| `info` | `info` | Display processor model, TSC frequency, and vendor ID. |
| `cpu` | `cpu` | Display CPU topology, extensions, and register states. |
| `security` | `security` | Display active mitigations (NX, SMEP, SMAP, UMIP, Stack Canaries). |
| `uptime` | `uptime` | System operating time since UEFI handover (`hh:mm:ss`). |
| `history` | `history` | List command history buffer. |
| `lastpanic` | `lastpanic` | Display diagnostic register dump from previous abnormal termination. |

### Memory & Storage
| Command | Usage | Description |
|:---|:---|:---|
| `mem` | `mem` | Physical RAM usage, frame availability, and kernel heap statistics. |
| `memmap` | `memmap` | Dumps UEFI physical memory descriptor map. |
| `alloc` | `alloc <pages>` | Allocates, tests, and frees physical memory frames. |
| `disks` | `disks` | List all attached physical and virtual disk drives. |
| `storage` | `storage` | Inspect AHCI SATA controller status, ports, and transfer modes. |
| `dread` | `dread <lba> [off]` | Read raw 512-byte sector data in hexadecimal format. |
| `dls` | `dls [path]` | List files directly from disk partition. |
| `dcat` | `dcat <file>` | Output raw file contents from storage volume. |

### Filesystem Operations
| Command | Usage | Description |
|:---|:---|:---|
| `write` | `write [-f] <file> <text>` | Write text to file (use `/usb/...` for permanent disk storage). |
| `append` | `append <file> <text>` | Append text line to file on storage. |
| `cat` | `cat <file>` | Display text file contents on terminal. |
| `ls` | `ls [path]` | List files and directories. |
| `tree` | `tree [path]` | Hierarchical directory tree view. |
| `cd` | `cd <path>` | Change active working directory. |
| `pwd` | `pwd` | Print current working directory path. |
| `mkdir` | `mkdir <dir>` | Create new directory. |
| `rmdir` | `rmdir <dir>` | Remove directory. |
| `rm` | `rm <file>` | Delete file. |
| `touch` | `touch <file>` | Create empty file or update timestamp. |
| `df` | `df` | Disk filesystem space utilization. |
| `mouse` | `mouse` | Pointer position and buttons (USB mouse or touchpad; wheel scrolls the log). |
| `touchpad` | `touchpad [scan\|rescan\|dump]` | The laptop's I2C touchpad: what the probe found, I2C bus scan, report descriptor. |

### Networking Subsystem
| Command | Usage | Description |
|:---|:---|:---|
| `connect` | `connect [eth\|wifi <ssid> <pwd>]` | Ethernet runs DHCP; Wi-Fi reports that there is no chip driver yet. |
| `wifi` | `wifi [status\|scan\|connect]` | Detects the Wi-Fi chip; no driver yet (see section 1). |
| `speedtest` | `speedtest [url]` | Downloads a real file (Debian's `Packages.xz`, ~4.5 MB, plain http) over the active link and prints bytes, seconds and the measured Mbit/s. Nothing is estimated. |
| `dhcp` | `dhcp [renew\|status]` | Auto-acquire IPv4 lease from gateway router (also brings up a newly plugged USB-tethered phone). |
| `ifconfig` | `ifconfig [if] [ip]` | Inspect or assign network interface IP configurations. |
| `ip` | `ip [a\|r\|l\|n]` | Linux-compatible IP address, route, and link viewer. |
| `ping` | `ping <target> [-c N]` | Transmit ICMP Echo Requests and measure round-trip latency. |
| `curl` | `curl <url> [-o file]` | Download over HTTP (DNS names, redirects, chunked). |
| `netstat` | `netstat` | Display active sockets, routes, and ARP table. |

### CPU Scheduling & Concurrency
| Command | Usage | Description |
|:---|:---|:---|
| `sched` | `sched [fcfs\|sjf\|srtf\|rr\|cmp]` | Compare and benchmark CPU process scheduling algorithms. |
| `ps` | `ps` | Display active tasks, process states, and CPU ticks. |
| `spawn` | `spawn <name>` | Create and enqueue background worker process. |
| `kill` | `kill <id>` | Terminate running task by PID. |
| `queue` | `queue` | View scheduler ready queue. |
| `job` | `job <burst> ...` | Launch concurrent burst tasks to evaluate priority scheduling. |
| `banker` | `banker [run\|detect]` | Dijkstra's Banker's algorithm safe-state matrix detection. |
| `diners` | `diners [steps]` | Multi-threaded Dining Philosophers synchronization simulation. |

### Algorithmic & Mathematical Coprocessor
| Command | Usage | Description |
|:---|:---|:---|
| `collatz` | `collatz <number>` | Compute hailstone convergence sequence and step count. |
| `pi` | `pi [samples]` | Monte Carlo statistical Pi estimation via hardware RDRAND. |
| `page` | `page [fifo\|lru\|optimal\|cmp]` | Virtual memory page replacement simulation and hit ratios. |
| `disksched`| `disksched [fcfs\|sstf\|scan\|cmp]` | Disk spindle seek optimization algorithm traversal comparison. |

### Text & Utility Tools
| Command | Usage | Description |
|:---|:---|:---|
| `echo` | `echo <text>` | Print text string to console. |
| `head` | `head [-n N] <file>` | Display first N lines of file. |
| `tail` | `tail [-n N] <file>` | Display final N lines of file. |
| `wc` | `wc <file>` | Count lines, words, and byte length of file. |
| `grep` | `grep <pattern> <file>` | Search for string patterns in text files. |
| `find` | `find [path] <name>` | Locate files matching filename pattern. |
| `hexdump`| `hexdump <file>` | Hexadecimal and ASCII dual-column dump. |
| `sort` | `sort <n1> <n2> ...` | Numerical sorting using heap memory. |
| `seq` | `seq [start] <end>` | Generate integer number sequence. |
| `sleep` | `sleep <seconds>` | Non-blocking kernel timer delay. |
| `clear` | `clear` | Clear terminal scrollback buffer (or Ctrl+L). |
| `reboot`| `reboot` | Hardware system reset. |
| `shutdown`| `shutdown` | ACPI power off transition (S5 state). |

### Programs, pipes and jobs (0.11.0, not yet booted: see docs/RELEASE-0.11.0.md)
| Syntax | Meaning |
|:---|:---|
| `hello`, `echo hi`, `cat file`, `wc file`, `ls /bin`, `forktest` | Programs in `/bin` (shipped inside the image) run as real processes. A name that is not a builtin is looked up in `/bin`, then `/usb/bin`; a path with `/` runs that file. |
| `a \| b \| c` | Pipe: all stages run at once. A builtin (such as `ps`) may be the first stage only. |
| `cmd > file`, `cmd >> file` | Send output to a file (replace / append). Works for builtins too. |
| `cmd &` | Run in the background; the shell announces when it ends. |
| `Ctrl+C` | End the foreground job. `Ctrl+D` on an empty line is end-of-input for the program. |
| `ps` | Lists kernel tasks, then programs with their pid and parent. |
| `kill <n>` | Ends the program with pid `n` (pids start at 100), else the kernel task with id `n`. |
| `date` | Date and time from the machine's clock chip (no time zone is applied). |
| `run <demo>` | The built-in ring-3 demos (`hello linux fault gp spin badcall denied`); several can run at once. |

---

## 6. Screenshots

The screenshots in `images/` were taken with earlier builds and show the old, simulated `apt` and Wi-Fi text: do not use them as evidence for 0.10.0. The QEMU test suite (`tests/boot_test.py`, groups `mouse`, `net`, `apt`, `usb`) is the evidence for the features above; it saves fresh screenshots to `target/test-shots/`.
