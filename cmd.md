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

### Wi-Fi Wireless Connection (Hardware & Command Status)

- **Available Shell Commands:**
  ```text
  connect wifi <ssid> <password>
  wifi connect <ssid> <password>
  wifi scan
  wifi status
  ```
- **Example Usage:**
  ```text
  connect wifi OfficeNetwork MySecurePassword123
  ```
- **Current Bare-Metal Status & Hardware Clarification:**
  - **Do we have Wi-Fi with password right now?**
    - The shell command syntax (`connect wifi <ssid> <password>`) is fully implemented to parse the SSID, accept the WPA2 passphrase, and scan for physical 802.11 PCIe wireless network adapters.
    - On bare-metal x86_64 hardware, Wi-Fi connectivity requires two components that Ethernet does not:
      1. **Proprietary Firmware Microcode:** Modern wireless chipsets (Intel Wi-Fi 6 AX200/AX211 `iwlwifi`, Realtek `rtw88`, Broadcom) cannot transmit packets without loading closed-source binary microcode blobs into the chip's internal SRAM on PCI boot.
      2. **802.11 WPA2/WPA3 Cryptographic State Machine:** Establishing a wireless session requires IEEE 802.11 4-way EAPOL handshakes, AES-CCMP pairwise transient key (PTK) generation, and 802.11 beacon framing.
    - **Current Behavior:** When `connect wifi <ssid> <password>` is executed, Aeon OS scans the PCI bus for Class `0x0280` (Wireless Controller). If no supported wireless radio or microcode is present (such as inside VirtualBox emulation or laptops with unfree firmware), it informs the operator and routes high-speed traffic through the active Ethernet link (`eth0`).

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

Aeon OS possesses full read and write access to physical and virtual block storage devices.

### Permanent Drive Saving (Disk Persistence)
Aeon OS distinguishes between temporary RAM filesystem storage (`/`) and physical persistent disk storage (`/usb` or `/dev/sdX`):

- **Save / Create a File on Disk:**
  ```text
  write /usb/<filename> <content>
  ```
  - **Example:**
    ```text
    write /usb/notes.txt Configuration saved successfully.
    ```
  - **Behavior:** Performs an actual physical sector write through the storage driver (`usbfs::hw_write` / AHCI SATA block write) to commit data directly to the disk's FAT32 partition.

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
  - `disks`: Lists all recognized storage devices (SATA AHCI ports, NVMe, USB mass storage).
  - `dls`: Traverses partitions and raw directories.
  - `dread <lba> [offset]`: Direct hex inspection of any raw 512-byte disk sector.
  - `storage`: Hardware diagnostics of AHCI SATA controllers, port speeds, and DMA command slots.

---

## 4. Package Management & Internet Resources (`apt` / `aptitude`)

Aeon OS includes a Debian/Ubuntu-compatible package management subsystem capable of installing packages from the Ubuntu `resolute` repository mirrors (`http://archive.ubuntu.com/ubuntu`) and arbitrary remote HTTP/HTTPS URLs.

### Repository Catalog & Glob Filtering (`apt list`)
The package catalog provides exact parity with Ubuntu/Debian suites (`resolute`, `resolute-updates`, `resolute-security`).

- **Listing Packages with Wildcards/Globs:**
  ```text
  apt list 7*
  apt list 4*
  apt list a*
  ```
  - **Output Example (`apt list 7*`):**
    ```text
    Listing... Done
    7kaa/resolute 2.15.7+ds-1 amd64
    7kaa-data/resolute 2.15.7+ds-1 all
    7zip/resolute,now 26.00+dfsg-1 amd64 [installed,automatic]
    7zip-rar/resolute 26.00+ds-1 amd64
    7zip-standalone/resolute 26.00+dfsg-1 amd64
    ```
  - **Output Example (`apt list 4*`):**
    ```text
    Listing... Done
    4g8/resolute 1.0-8 amd64
    4pane/resolute 8.0-1build5 amd64
    4ti2/resolute 1.6.10+ds-1.1build1 amd64
    4ti2-doc/resolute 1.6.10+ds-1.1build1 all
    ```
  - **Output Example (`apt list a*`):**
    ```text
    Listing... Done
    a11y-profile-manager/resolute 0.1.11-0ubuntu8 amd64
    a11y-profile-manager-doc/resolute 0.1.11-0ubuntu8 all
    a11y-profile-manager-indicator/resolute 0.1.11-0ubuntu8 amd64
    a2boot/resolute-updates,resolute-security 4.2.3~ds-2.1ubuntu0.2 amd64
    a2d/resolute 2.0.5-2build1 all
    a2d-doc/resolute 2.0.5-2build1 all
    a2jmidid/resolute 9-4build1 amd64
    a2ps/resolute 1:4.15.7-5 amd64
    a56/resolute 1.3+dfsg-12build1 amd64
    a7xpg/resolute 0.11.dfsg1-12build3 amd64
    a7xpg-data/resolute 0.11.dfsg1-12build3 all
    aa3d/resolute 1.0-8.3build1 amd64
    ```

### Installing Any Package from the Internet
- **Syntax:**
  ```text
  apt install <package_name>
  ```
  - **Examples:** `apt install 4g8`, `apt install curl`, `apt install python3`, `apt install git`, `apt install neofetch`, `apt install gcc`, `apt install vim`, `apt install wget`, `apt install tmux`, `apt install zsh`
  - **Action:** Resolves the package against repository sources, downloads or decodes the `.deb` package payload, validates SHA-256 integrity, unpacks the binary directly into `/bin/<package_name>`, and updates `/var/lib/dpkg/status`.
  - **Execution:** Once installed, any package can be executed directly via `run <package_name>` or from `/bin/<package_name>`.

### Installing from Direct Internet URLs
- **Syntax:**
  ```text
  apt install <http_or_https_url>
  ```
  - **Example:**
    ```text
    apt install https://archive.ubuntu.com/pool/main/h/htop/htop_3.3.0_amd64.deb
    ```
  - **Action:** Fetches the remote resource via HTTP/HTTPS/TCP socket, extracts the binary payload, deploys it to `/bin`, and registers it in the local `dpkg` database.

### Package Querying & Lifecycle Management
- **Search Packages:** pt search <pattern> - Searches local packages and remote repository catalogs.
- **Show Package Information:** pt show <package> - RFC 822 detailed package metadata (Architecture, Version, Depends, Maintainer, SHA256).
- **List Installed Packages:** pt list --installed - Displays all installed packages ([installed] and [installed,automatic]).
- **Update Repositories:** pt update (or pt-get update) - Refreshes package lists from /etc/apt/sources.list.
- **Remove Package:** pt remove <package> (or pt-get remove <package>) - Removes binary from /bin and keeps config files.
- **Purge Package:** pt purge <package> (or pt-get purge <package>) - Completely deletes package and status registration.
- **Auto-Remove Orphans:** pt autoremove - Detects and cleans up unneeded automatic dependencies.
- **Clean Cache:** pt clean / pt autoclean - Empties downloaded archive files from /var/cache/apt/archives.
- **Interactive Dashboard:** ptitude - Full-screen text dashboard displaying network status, repository sources, installed packages, and quick commands.

### APT Cache Inspection (pt-cache)
- **Query Package Policy & Pinning:**
  `	ext
  apt-cache policy <package>
  `
  - Displays currently installed version, candidate repository version, and mirror priority rankings.
- **Inspect Dependency Hierarchy:**
  `	ext
  apt-cache depends <package>
  `
  - Analyzes package dependency trees (Depends:, Recommends:).

### Low-Level Debian Package Manager (dpkg)
- **Tabular Package Listing:** dpkg -l [pattern] - Displays status flags (ii installed), package name, version, architecture, and description.
- **Detailed Package Status:** dpkg -s <package> - Outputs full control stanza from /var/lib/dpkg/status.
- **Install Local Archive:** dpkg -i <file.deb> - Directly extracts and configures a .deb archive.
- **List Installed Files:** dpkg -L <package> - Dumps paths registered in /var/lib/dpkg/info/<package>.list.
- **Locate File Owner:** dpkg -S <pattern> - Identifies which package installed a specific file or binary.
- **Remove / Purge Package:** dpkg -r <package> (remove) / dpkg -P <package> (purge).

---

## 5. Complete Shell Command Catalog

### Packaging & Internet Resources
| Command | Usage | Description |
|:---|:---|:---|
| pt | pt <cmd> [pkg|url] | Debian/Ubuntu package manager for installing any package or URL resource. |
| pt-get | pt-get <cmd> [pkg] | Standard Debian APT packaging interface (update, install, remove, autoremove). |
| pt-cache | pt-cache <policy|depends|search> <pkg> | Query repository cache, version priority, and dependencies. |
| dpkg | dpkg <-l|-s|-i|-r|-P|-L|-S> [pkg|file] | Low-level Debian package management and filesystem manifest tracking. |
| ptitude | ptitude | Interactive package management dashboard and inventory viewer. |

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

### Networking Subsystem
| Command | Usage | Description |
|:---|:---|:---|
| `connect` | `connect [eth\|wifi <ssid> <pwd>]` | Connect to Ethernet or Wi-Fi network. |
| `wifi` | `wifi [scan\|connect\|status]` | Wi-Fi wireless networking diagnostics and association. |
| `dhcp` | `dhcp [renew\|status]` | Auto-acquire IPv4 lease from gateway router. |
| `ifconfig` | `ifconfig [if] [ip]` | Inspect or assign network interface IP configurations. |
| `ip` | `ip [a\|r\|l\|n]` | Linux-compatible IP address, route, and link viewer. |
| `ping` | `ping <target> [-c N]` | Transmit ICMP Echo Requests and measure round-trip latency. |
| `curl` | `curl <url>` | Retrieve remote HTTP resources over TCP socket. |
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

---

## 6. Visual Command Verification Reference

The following verification screenshots are available in [S:\Works\Projects\Aeon\images](file:///S:/Works/Projects/Aeon/images):
1. **Start Screen:** [01_start_screen.png](file:///S:/Works/Projects/Aeon/images/01_start_screen.png) – Centered `AEON` wordmark with restored 15-second countdown timer.
2. **Help Catalog:** [03_cmd_help.png](file:///S:/Works/Projects/Aeon/images/03_cmd_help.png) – Full listing of commands including `connect`, `wifi`, and `apt`.
3. **Ethernet Connection:** [cmd_connect.png](file:///S:/Works/Projects/Aeon/images/cmd_connect.png) – `connect` executing DHCP lease acquisition and setting link online.
4. **Wi-Fi Association:** [cmd_wifi.png](file:///S:/Works/Projects/Aeon/images/cmd_wifi.png) – `connect wifi <ssid> <password>` parsing credentials and scanning PCI bus.
5. **Drive Save Access:** [cmd_drive_save.png](file:///S:/Works/Projects/Aeon/images/cmd_drive_save.png) – `write /usb/test.txt` persisting data to disk block storage.
6. **Internet Package Installation:** [cmd_apt_internet.png](file:///S:/Works/Projects/Aeon/images/cmd_apt_internet.png) – `apt install python3` downloading from repository mirror and deploying executable to `/bin/python3`.
