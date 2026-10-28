import os
import struct
import subprocess

SECTOR_SIZE = 512
TOTAL_SECTORS = 67584 # 33 MB
RESERVED_SECTORS = 4
NUM_FATS = 2
SECTORS_PER_FAT = 64
ROOT_ENTRIES = 512
ROOT_DIR_SECTORS = (ROOT_ENTRIES * 32) // SECTOR_SIZE # 32 sectors = 16384 bytes
CLUSTER_SIZE = 2048 # 4 sectors
SECTORS_PER_CLUSTER = 4

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
EFI_BIN_PATH = os.path.join(BASE_DIR, "disk", "EFI", "BOOT", "BOOTX64.EFI")
IMG_PATH = os.path.join(BASE_DIR, "aeon.img")
NEW_VDI_PATH = os.path.join(BASE_DIR, "aeon_new.vdi")
FINAL_VDI_PATH = os.path.join(BASE_DIR, "aeon.vdi")
VBOX_UUID = "8a0bb14a-6918-451d-9600-bfe20d28a175"

with open(EFI_BIN_PATH, "rb") as f:
    efi_data = f.read()

efi_size = len(efi_data)
efi_clusters = (efi_size + CLUSTER_SIZE - 1) // CLUSTER_SIZE

print(f"Staging BOOTX64.EFI: {efi_size} bytes ({efi_clusters} clusters)")

img = bytearray(TOTAL_SECTORS * SECTOR_SIZE)

# Boot sector (Sector 0)
bs = bytearray(512)
bs[0:3] = b'\xeb\x3c\x90'
bs[3:11] = b'MSWIN4.1'
struct.pack_into('<H', bs, 11, SECTOR_SIZE)
bs[13] = SECTORS_PER_CLUSTER
struct.pack_into('<H', bs, 14, RESERVED_SECTORS)
bs[16] = NUM_FATS
struct.pack_into('<H', bs, 17, ROOT_ENTRIES)
struct.pack_into('<H', bs, 19, 0)
bs[21] = 0xF8 # Media descriptor
struct.pack_into('<H', bs, 22, SECTORS_PER_FAT)
struct.pack_into('<H', bs, 24, 63)
struct.pack_into('<H', bs, 26, 16)
struct.pack_into('<I', bs, 28, 0)
struct.pack_into('<I', bs, 32, TOTAL_SECTORS)
bs[36] = 0x80
bs[38] = 0x29
struct.pack_into('<I', bs, 39, 0x12345678)
bs[43:54] = b'NO NAME    '
bs[54:62] = b'FAT16   '
bs[510:512] = b'\x55\xaa'
img[0:512] = bs

# Setup FAT tables
fat = bytearray(SECTORS_PER_FAT * SECTOR_SIZE)
struct.pack_into('<H', fat, 0, 0xFFF8)
struct.pack_into('<H', fat, 2, 0xFFFF)
struct.pack_into('<H', fat, 4, 0xFFFF) # Cluster 2 (EFI dir)
struct.pack_into('<H', fat, 6, 0xFFFF) # Cluster 3 (BOOT dir)

cur_cluster = 4
for i in range(efi_clusters):
    if i == efi_clusters - 1:
        next_c = 0xFFFF
    else:
        next_c = cur_cluster + 1
    struct.pack_into('<H', fat, cur_cluster * 2, next_c)
    cur_cluster += 1

fat_offset = RESERVED_SECTORS * SECTOR_SIZE
fat_size = len(fat)
img[fat_offset : fat_offset + fat_size] = fat
fat2_offset = fat_offset + fat_size
img[fat2_offset : fat2_offset + fat_size] = fat

# Root directory
root_offset = fat2_offset + fat_size
def make_entry(name, ext, attr, cluster, size):
    e = bytearray(32)
    e[0:8] = name.ljust(8).encode('ascii')
    e[8:11] = ext.ljust(3).encode('ascii')
    e[11] = attr
    struct.pack_into('<H', e, 26, cluster)
    struct.pack_into('<I', e, 28, size)
    return e

img[root_offset : root_offset + 32] = make_entry('EFI', '', 0x10, 2, 0)

data_offset = root_offset + ROOT_DIR_SECTORS * SECTOR_SIZE
def get_c_off(c): return data_offset + (c - 2) * CLUSTER_SIZE

# Cluster 2: EFI directory
c2 = bytearray(CLUSTER_SIZE)
c2[0:32] = make_entry('.', '', 0x10, 2, 0)
c2[32:64] = make_entry('..', '', 0x10, 0, 0)
c2[64:96] = make_entry('BOOT', '', 0x10, 3, 0)
off2 = get_c_off(2)
img[off2 : off2 + CLUSTER_SIZE] = c2

# Cluster 3: BOOT directory
c3 = bytearray(CLUSTER_SIZE)
c3[0:32] = make_entry('.', '', 0x10, 3, 0)
c3[32:64] = make_entry('..', '', 0x10, 2, 0)
c3[64:96] = make_entry('BOOTX64', 'EFI', 0x20, 4, efi_size)
off3 = get_c_off(3)
img[off3 : off3 + CLUSTER_SIZE] = c3

# Cluster 4+: BOOTX64.EFI binary payload
off4 = get_c_off(4)
img[off4 : off4 + len(efi_data)] = efi_data

with open(IMG_PATH, "wb") as f:
    f.write(img)

print(f"Generated raw FAT16 image: {IMG_PATH} ({len(img)} bytes)")

# Convert to VDI using VBoxManage
if os.path.exists(NEW_VDI_PATH):
    os.remove(NEW_VDI_PATH)

vboxmanage = r"C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"
cmd = [vboxmanage, "convertfromraw", IMG_PATH, NEW_VDI_PATH, "--format", "VDI", "--uuid", VBOX_UUID]
print("Running VBoxManage convertfromraw...")
res = subprocess.run(cmd, capture_output=True, text=True)
if res.returncode != 0:
    print("VBoxManage convert error:", res.stderr)
    raise RuntimeError(res.stderr)

print("Conversion successful.")

# Atomic replace of aeon.vdi
if os.path.exists(FINAL_VDI_PATH):
    os.remove(FINAL_VDI_PATH)
os.rename(NEW_VDI_PATH, FINAL_VDI_PATH)
os.remove(IMG_PATH)
print(f"Updated {FINAL_VDI_PATH} with UUID {VBOX_UUID}.")
