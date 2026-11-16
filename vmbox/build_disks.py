#!/usr/bin/env python3
"""
Builds the bootable EFI virtual disk (Aeon-Boot.vdi) and optional demo FAT32 disk (Aeon-DemoDisk.vdi)
for Oracle VM VirtualBox.
"""
import os
import sys
import uuid
import zlib
import struct
import subprocess

SECTOR = 512

def lfn_entries(long_name, short11):
    chk = 0
    for c in short11:
        chk = (((chk & 1) << 7) + (chk >> 1) + c) & 0xFF
    units = [ord(c) for c in long_name] + [0]
    while len(units) % 13:
        units.append(0xFFFF)
    parts = [units[i:i + 13] for i in range(0, len(units), 13)]
    out = []
    for n in range(len(parts), 0, -1):
        u = parts[n - 1]
        seq = n | (0x40 if n == len(parts) else 0)
        e = (bytes([seq]) +
             struct.pack("<5H", *u[0:5]) +
             bytes([0x0F, 0, chk]) +
             struct.pack("<6H", *u[5:11]) +
             b"\0\0" +
             struct.pack("<2H", *u[11:13]))
        assert len(e) == 32
        out.append(e)
    return out

def make_dirent(name11, attr, cluster, size):
    e = bytearray(32)
    e[0:11] = name11
    e[11] = attr
    struct.pack_into("<H", e, 20, (cluster >> 16) & 0xFFFF)
    struct.pack_into("<H", e, 26, cluster & 0xFFFF)
    struct.pack_into("<I", e, 28, size)
    return bytes(e)

def build_fat32_partition(total_sectors, files):
    spc = 1
    reserved = 32
    
    fat_sectors = 1
    while True:
        clusters = (total_sectors - reserved - 2 * fat_sectors) // spc
        need = ((clusters + 2) * 4 + SECTOR - 1) // SECTOR
        if need <= fat_sectors:
            break
        fat_sectors += 1

    assert clusters >= 65525, f"Not enough clusters for FAT32: {clusters}"

    part = bytearray(total_sectors * SECTOR)

    # Boot Sector
    bs = bytearray(SECTOR)
    bs[0:3] = b"\xEB\x58\x90"
    bs[3:11] = b"AEONFAT3"
    struct.pack_into("<HBHBHHBHHHII", bs, 11,
                     SECTOR, spc, reserved, 2, 0, 0, 0xF8, 0, 32, 64, 2048, total_sectors)
    struct.pack_into("<IHHIHH", bs, 36, fat_sectors, 0, 0, 2, 1, 6)
    bs[66] = 0x29
    struct.pack_into("<I", bs, 67, 0x19980512)
    bs[71:82] = b"AEON BOOT  "
    bs[82:90] = b"FAT32   "
    bs[510:512] = b"\x55\xAA"
    part[0:SECTOR] = bs
    part[6 * SECTOR:7 * SECTOR] = bs # Backup boot sector

    # FSInfo
    fsinfo = bytearray(SECTOR)
    fsinfo[0:4] = b"RRaA"
    fsinfo[484:488] = b"rrAa"
    struct.pack_into("<II", fsinfo, 488, 0xFFFFFFFF, 3)
    fsinfo[508:512] = b"\x00\x00\x55\xAA"
    part[SECTOR:2 * SECTOR] = fsinfo
    part[7 * SECTOR:8 * SECTOR] = fsinfo

    fat_off = reserved * SECTOR
    first_data = reserved + 2 * fat_sectors
    next_cluster = [3]

    def fat_set(c, v):
        for k in range(2):
            o = fat_off + k * fat_sectors * SECTOR + c * 4
            struct.pack_into("<I", part, o, v)

    fat_set(0, 0x0FFFFFF8)
    fat_set(1, 0x0FFFFFFF)
    fat_set(2, 0x0FFFFFFF)

    def cluster_off(c):
        return (first_data + (c - 2) * spc) * SECTOR

    def alloc_chain(data):
        if not data:
            return 0
        n = (len(data) + spc * SECTOR - 1) // (spc * SECTOR)
        first = next_cluster[0]
        cl = list(range(first, first + n))
        next_cluster[0] += n
        for i, c in enumerate(cl):
            fat_set(c, cl[i + 1] if i + 1 < n else 0x0FFFFFFF)
            chunk = data[i * spc * SECTOR:(i + 1) * spc * SECTOR]
            part[cluster_off(c):cluster_off(c) + len(chunk)] = chunk
        return first

    bootx64_c = alloc_chain(files['bootx64_efi'])
    cfg_c = alloc_chain(files.get('aeon_cfg')) if files.get('aeon_cfg') else 0
    sha_c = alloc_chain(files['sha256_txt'])
    cert_c = alloc_chain(files['cert'])

    # /EFI/BOOT
    boot_c = next_cluster[0]
    next_cluster[0] += 1
    fat_set(boot_c, 0x0FFFFFFF)

    # /EFI
    efi_c = next_cluster[0]
    next_cluster[0] += 1
    fat_set(efi_c, 0x0FFFFFFF)

    # Fill /EFI/BOOT
    boot_dir = bytearray()
    boot_dir += make_dirent(b".          ", 0x10, boot_c, 0)
    boot_dir += make_dirent(b"..         ", 0x10, efi_c, 0)
    boot_dir += make_dirent(b"BOOTX64 EFI", 0x20, bootx64_c, len(files['bootx64_efi']))
    if cfg_c:
        boot_dir += make_dirent(b"AEON    CFG", 0x20, cfg_c, len(files['aeon_cfg']))
    part[cluster_off(boot_c):cluster_off(boot_c) + len(boot_dir)] = boot_dir

    # Fill /EFI
    efi_dir = bytearray()
    efi_dir += make_dirent(b".          ", 0x10, efi_c, 0)
    efi_dir += make_dirent(b"..         ", 0x10, 0, 0)
    efi_dir += make_dirent(b"BOOT       ", 0x10, boot_c, 0)
    part[cluster_off(efi_c):cluster_off(efi_c) + len(efi_dir)] = efi_dir

    # Fill Root directory
    root = bytearray()
    root += make_dirent(b"AEON BOOT  ", 0x08, 0, 0)
    root += make_dirent(b"EFI        ", 0x10, efi_c, 0)
    root += make_dirent(b"SHA256  TXT", 0x20, sha_c, len(files['sha256_txt']))
    cert_alias = b"AEON-S~1CER"
    for e in lfn_entries("aeon-secureboot.cer", cert_alias):
        root += e
    root += make_dirent(cert_alias, 0x20, cert_c, len(files['cert']))
    part[cluster_off(2):cluster_off(2) + len(root)] = root

    return part

def build_boot_disk(out_img_path, files):
    total_disk_sectors = 131072 # 64 MB
    esp_lba_start = 2048
    esp_lba_end = total_disk_sectors - 34
    esp_sectors = esp_lba_end - esp_lba_start + 1

    fat_data = build_fat32_partition(esp_sectors, files)

    disk = bytearray(total_disk_sectors * SECTOR)

    # 1. Protective / Hybrid MBR
    mbr = bytearray(SECTOR)
    mbr[446] = 0x80 # Bootable
    mbr[447:450] = b"\x00\x20\x21"
    mbr[450] = 0xEF # EFI System Partition
    mbr[451:454] = b"\xFE\xFF\xFF"
    struct.pack_into("<II", mbr, 454, esp_lba_start, esp_sectors)
    mbr[510:512] = b"\x55\xAA"
    disk[0:SECTOR] = mbr

    # 2. Primary GPT
    disk_guid = uuid.uuid4().bytes_le
    part_guid = uuid.uuid4().bytes_le
    esp_type_guid = uuid.UUID("{C12A7328-F81F-11D2-BA4B-00A0C93EC93B}").bytes_le

    part_entry = bytearray(128)
    part_entry[0:16] = esp_type_guid
    part_entry[16:32] = part_guid
    struct.pack_into("<QQQ", part_entry, 32, esp_lba_start, esp_lba_end, 0)
    name_utf16 = "EFI System Partition".encode("utf-16le")
    part_entry[56:56 + len(name_utf16)] = name_utf16

    part_table = bytearray(128 * 128)
    part_table[0:128] = part_entry
    part_crc = zlib.crc32(part_table) & 0xFFFFFFFF

    gpt_hdr = bytearray(92)
    gpt_hdr[0:8] = b"EFI PART"
    struct.pack_into("<II", gpt_hdr, 8, 0x00010000, 92)
    struct.pack_into("<I", gpt_hdr, 20, 0)
    struct.pack_into("<QQQQ", gpt_hdr, 24, 1, total_disk_sectors - 1, 2048, total_disk_sectors - 34)
    gpt_hdr[56:72] = disk_guid
    struct.pack_into("<QII", gpt_hdr, 72, 2, 128, 128)
    struct.pack_into("<I", gpt_hdr, 88, part_crc)
    hdr_crc = zlib.crc32(gpt_hdr) & 0xFFFFFFFF
    struct.pack_into("<I", gpt_hdr, 16, hdr_crc)

    disk[SECTOR:2 * SECTOR] = gpt_hdr + bytearray(SECTOR - 92)
    disk[2 * SECTOR:34 * SECTOR] = part_table

    # 3. Partition Data
    disk[esp_lba_start * SECTOR:(esp_lba_start + esp_sectors) * SECTOR] = fat_data

    # 4. Backup GPT
    disk[(total_disk_sectors - 33) * SECTOR:(total_disk_sectors - 1) * SECTOR] = part_table

    gpt_bak = bytearray(gpt_hdr)
    struct.pack_into("<I", gpt_bak, 16, 0)
    struct.pack_into("<QQ", gpt_bak, 24, total_disk_sectors - 1, 1)
    struct.pack_into("<Q", gpt_bak, 72, total_disk_sectors - 33)
    bak_crc = zlib.crc32(gpt_bak) & 0xFFFFFFFF
    struct.pack_into("<I", gpt_bak, 16, bak_crc)
    disk[(total_disk_sectors - 1) * SECTOR:total_disk_sectors * SECTOR] = gpt_bak + bytearray(SECTOR - 92)

    with open(out_img_path, "wb") as f:
        f.write(disk)

BOOT_UUID = "3505efa3-ce2e-4b9c-8047-3fa6ff599e82"
DEMO_UUID = "47b5a9a9-b5ee-4139-9545-cffd0c15e638"

def main():
    vm_dir = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.path.join(vm_dir, "..", "OS", "usb"),
        os.path.join(vm_dir, "..", "usb"),
        os.path.join(vm_dir, "usb"),
        r"S:\Works\Projects\Aeon\OS\usb",
        r"G:\Aeon\OS\usb",
    ]
    aeon_usb = None
    for cand in candidates:
        if os.path.exists(os.path.join(cand, r"EFI\BOOT\BOOTX64.EFI")):
            aeon_usb = os.path.abspath(cand)
            break
    if not aeon_usb:
        print("ERROR: Could not locate Aeon usb directory containing EFI\\BOOT\\BOOTX64.EFI")
        sys.exit(1)

    print("Reading Aeon bootloader files from:", aeon_usb)
    with open(os.path.join(aeon_usb, r"EFI\BOOT\BOOTX64.EFI"), "rb") as f:
        bootx64 = f.read()
    with open(os.path.join(aeon_usb, "SHA256.txt"), "rb") as f:
        sha = f.read()
    with open(os.path.join(aeon_usb, "aeon-secureboot.cer"), "rb") as f:
        cert = f.read()

    files = {
        'bootx64_efi': bootx64,
        'sha256_txt': sha,
        'cert': cert,
        'aeon_cfg': b"# Aeon Boot Configuration\nmode=1024x768\ntimeout=5\n"
    }

    boot_raw = os.path.join(vm_dir, "Aeon-Boot.raw")
    boot_vdi = os.path.join(vm_dir, "Aeon-Boot.vdi")
    print("Building raw boot disk:", boot_raw)
    build_boot_disk(boot_raw, files)

    vboxmanage = r"C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"
    if os.path.exists(boot_vdi):
        os.remove(boot_vdi)
    print("Converting to VDI with fixed UUID:", boot_vdi)
    subprocess.run([vboxmanage, "convertfromraw", boot_raw, boot_vdi, "--format", "VDI", f"--uuid={BOOT_UUID}"], check=True)
    if os.path.exists(boot_raw):
        os.remove(boot_raw)
    print("Aeon-Boot.vdi created successfully.")

    # Generate Demo Disk
    mkdisk_path = os.path.join(vm_dir, "..", "tests", "mkdisk.py")
    if not os.path.exists(mkdisk_path):
        mkdisk_path = r"S:\Works\Projects\Aeon\OS\tests\mkdisk.py"
    if not os.path.exists(mkdisk_path):
        mkdisk_path = r"G:\Aeon\OS\tests\mkdisk.py"
    if os.path.exists(mkdisk_path):
        demo_raw = os.path.join(vm_dir, "Aeon-DemoDisk.raw")
        demo_vdi = os.path.join(vm_dir, "Aeon-DemoDisk.vdi")
        print("Building demo disk:", demo_raw)
        subprocess.run([sys.executable, mkdisk_path, demo_raw, "--fat32"], check=True)
        if os.path.exists(demo_vdi):
            os.remove(demo_vdi)
        print("Converting to VDI with fixed UUID:", demo_vdi)
        subprocess.run([vboxmanage, "convertfromraw", demo_raw, demo_vdi, "--format", "VDI", f"--uuid={DEMO_UUID}"], check=True)
        if os.path.exists(demo_raw):
            os.remove(demo_raw)
        print("Aeon-DemoDisk.vdi created successfully.")

    print("\nAll virtual disks built in:", vm_dir)

if __name__ == "__main__":
    main()
