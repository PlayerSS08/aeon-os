#!/usr/bin/env python3
"""Builds small FAT16 or FAT32 disk images for the storage tests. No tools needed, no root.

    python3 tests/mkdisk.py OUT.img [--fat32]

Contents (same for both types):
  HELLO.TXT          "hello from the disk\n"
  BIG.BIN            3000 bytes: byte i is i % 251 (spans several clusters)
  Long File Name.txt "long names are skipped but their short alias works\n" (short alias LONGFI~1.TXT)
  DOCS/NOTE.TXT      "note in a subdirectory\n"
"""
import struct
import sys

SECTOR = 512


def lfn_entries(long_name, short11):
    """Long-file-name directory entries (last part first) for `long_name`, ending in the 8.3 alias."""
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
        e = bytes([seq]) + struct.pack("<5H", *u[0:5]) + bytes([0x0F, 0, chk]) + struct.pack("<6H", *u[5:11]) + b"\0\0" + struct.pack("<2H", *u[11:13])
        assert len(e) == 32
        out.append(e)
    return out


def make_dirent(name11, attr, cluster, size):
    """A 32-byte short directory entry."""
    e = bytearray(32)
    e[0:11] = name11
    e[11] = attr
    struct.pack_into("<H", e, 20, (cluster >> 16) & 0xFFFF)  # first cluster, high word
    struct.pack_into("<H", e, 26, cluster & 0xFFFF)          # first cluster, low word
    struct.pack_into("<I", e, 28, size)
    return bytes(e)


def build(fat32):
    total_sectors = 70000 if fat32 else 8192
    spc = 1
    reserved = 32 if fat32 else 1
    root_entries = 0 if fat32 else 512
    root_sectors = (root_entries * 32 + SECTOR - 1) // SECTOR
    # find FAT size: clusters = (total - reserved - root - 2*fat) / spc
    fat_sectors = 1
    while True:
        clusters = (total_sectors - reserved - root_sectors - 2 * fat_sectors) // spc
        need = ((clusters + 2) * (4 if fat32 else 2) + SECTOR - 1) // SECTOR
        if need <= fat_sectors:
            break
        fat_sectors += 1
    if fat32:
        assert clusters >= 65525, clusters
    else:
        assert 4085 <= clusters < 65525, clusters
    img = bytearray(total_sectors * SECTOR)

    bs = bytearray(SECTOR)
    bs[0:3] = b"\xEB\x58\x90" if fat32 else b"\xEB\x3C\x90"
    bs[3:11] = b"AEONMKFS"
    struct.pack_into("<HBHBHHBHHHII", bs, 11, SECTOR, spc, reserved, 2, root_entries, total_sectors if total_sectors < 65536 else 0, 0xF8, 0 if fat32 else fat_sectors, 32, 64, 0, total_sectors if total_sectors >= 65536 else 0)
    if fat32:
        struct.pack_into("<IHHIHH", bs, 36, fat_sectors, 0, 0, 2, 1, 6)
        bs[66] = 0x29
        bs[71:82] = b"AEON DISK  "
        bs[82:90] = b"FAT32   "
    else:
        bs[36] = 0x80
        bs[38] = 0x29
        bs[43:54] = b"AEON DISK  "
        bs[54:62] = b"FAT16   "
    bs[510:512] = b"\x55\xAA"
    img[0:SECTOR] = bs

    fat_off = reserved * SECTOR
    fat_bytes = 4 if fat32 else 2
    first_data = reserved + 2 * fat_sectors + root_sectors
    next_cluster = [2 if not fat32 else 3]  # FAT32 uses cluster 2 for the root directory

    def fat_set(c, v):
        for k in range(2):
            o = fat_off + k * fat_sectors * SECTOR + c * fat_bytes
            if fat32:
                struct.pack_into("<I", img, o, v)
            else:
                struct.pack_into("<H", img, o, v)

    eoc = 0x0FFFFFF8 if fat32 else 0xFFF8
    fat_set(0, 0x0FFFFFF8 if fat32 else 0xFFF8)
    fat_set(1, 0x0FFFFFFF if fat32 else 0xFFFF)

    def cluster_off(c):
        return (first_data + (c - 2) * spc) * SECTOR

    def alloc_chain(data):
        n = max(1, (len(data) + spc * SECTOR - 1) // (spc * SECTOR)) if data else 0
        if n == 0:
            return 0
        first = next_cluster[0]
        cl = list(range(first, first + n))
        next_cluster[0] += n
        for i, c in enumerate(cl):
            fat_set(c, cl[i + 1] if i + 1 < n else eoc)
            chunk = data[i * spc * SECTOR:(i + 1) * spc * SECTOR]
            img[cluster_off(c):cluster_off(c) + len(chunk)] = chunk
        return first

    # DOCS directory
    note = b"note in a subdirectory\n"
    note_c = alloc_chain(note)
    docs_dir = bytearray()
    docs_dir += make_dirent(b".          ", 0x10, next_cluster[0], 0)  # "." names the directory itself
    docs_dir += make_dirent(b"..         ", 0x10, 0, 0)
    docs_dir += make_dirent(b"NOTE    TXT", 0x20, note_c, len(note))
    docs_c = alloc_chain(bytes(docs_dir))

    hello = b"hello from the disk\n"
    big = bytes(i % 251 for i in range(3000))
    longf = b"long names are skipped but their short alias works\n"
    root = bytearray()
    root += make_dirent(b"AEON DISK  ", 0x08, 0, 0)  # volume label
    root += make_dirent(b"HELLO   TXT", 0x20, alloc_chain(hello), len(hello))
    root += make_dirent(b"BIG     BIN", 0x20, alloc_chain(big), len(big))
    alias = b"LONGFI~1TXT"
    for e in lfn_entries("Long File Name.txt", alias):
        root += e
    root += make_dirent(alias, 0x20, alloc_chain(longf), len(longf))
    root += make_dirent(b"DOCS       ", 0x10, docs_c, 0)
    if fat32:
        root_c = 2
        fat_set(2, eoc)
        img[cluster_off(2):cluster_off(2) + len(root)] = root
    else:
        o = (reserved + 2 * fat_sectors) * SECTOR
        img[o:o + len(root)] = root
    if fat32:
        # FSInfo is optional for a read-only reader; the BPB above names sector 1, so leave it zeroed.
        pass
    return bytes(img)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    data = build("--fat32" in sys.argv[2:])
    with open(out, "wb") as f:
        f.write(data)
    print(f"wrote {out}: {len(data)} bytes, {'FAT32' if '--fat32' in sys.argv[2:] else 'FAT16'}")


if __name__ == "__main__":
    main()
