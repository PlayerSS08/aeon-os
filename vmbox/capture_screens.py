import os
import sys
import time
import shutil
import subprocess

VBOXMANAGE = r"C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"
VM_NAME = "Aeon-VM"
OUT_DIR = r"S:\Works\Projects\Aeon\OS\docs\screenshots"
ARTIFACT_DIR = r"C:\Users\SREEHARAN\.gemini\antigravity-cli\brain\a31e20c8-d42d-4638-a87b-72462ab4bd5e"

SCANCODES = {
    'a': '1e 9e', 'b': '30 b0', 'c': '2e ae', 'd': '20 a0', 'e': '12 92',
    'f': '21 a1', 'g': '22 a2', 'h': '23 a3', 'i': '17 97', 'j': '24 a4',
    'k': '25 a5', 'l': '26 a6', 'm': '32 b2', 'n': '31 b1', 'o': '18 98',
    'p': '19 99', 'q': '10 90', 'r': '13 93', 's': '1f 9f', 't': '14 94',
    'u': '16 96', 'v': '2f af', 'w': '11 91', 'x': '2d ad', 'y': '15 95',
    'z': '2c ac',
    '0': '0b 8b', '1': '02 82', '2': '03 83', '3': '04 84', '4': '05 85',
    '5': '06 86', '6': '07 87', '7': '08 88', '8': '09 89', '9': '0a 8a',
    ' ': '39 b9', '-': '0c 8c', '.': '34 b4', '/': '35 b5', '\n': '1c 9c',
    ':': '2a 27 a7 aa', # shift + ;
}

def run_vbox(args):
    return subprocess.run([VBOXMANAGE] + args, capture_output=True, text=True)

def poweroff():
    run_vbox(["controlvm", VM_NAME, "poweroff"])
    time.sleep(1)

def send_text(text, delay=0.04):
    for ch in text:
        low = ch.lower()
        if low in SCANCODES:
            code = SCANCODES[low]
            run_vbox(["controlvm", VM_NAME, "keyboardputscancode"] + code.split())
            time.sleep(delay)

def send_ctrl_l():
    # Ctrl down (1d), L down (26), L up (a6), Ctrl up (9d)
    run_vbox(["controlvm", VM_NAME, "keyboardputscancode", "1d", "26", "a6", "9d"])
    time.sleep(0.2)

def capture(name):
    os.makedirs(OUT_DIR, exist_ok=True)
    os.makedirs(ARTIFACT_DIR, exist_ok=True)
    dst1 = os.path.join(OUT_DIR, name)
    dst2 = os.path.join(ARTIFACT_DIR, name)
    run_vbox(["controlvm", VM_NAME, "screenshotpng", dst1])
    if os.path.exists(dst1):
        shutil.copy2(dst1, dst2)
        print(f"Captured {name}: {os.path.getsize(dst1)} bytes")
    else:
        print(f"Failed to capture {name}")

def main():
    print("Stopping any running instance...")
    poweroff()

    print("Starting Aeon-VM...")
    run_vbox(["startvm", VM_NAME, "--type", "headless"])

    # 1. Start Screen / Boot Menu
    print("Waiting for boot menu...")
    time.sleep(1.8)
    capture("01_start_screen.png")

    # 2. Boot to Shell
    print("Waiting for boot to finish into shell...")
    time.sleep(4.5)
    capture("02_shell_initial.png")

    # 3. Help Command Page
    print("Running: help")
    send_text("help\n")
    time.sleep(1.2)
    capture("03_cmd_help.png")

    # 4. System Status
    print("Running: status")
    send_ctrl_l()
    send_text("status\n")
    time.sleep(1.2)
    capture("04_cmd_status.png")

    # 5. Hardware Info
    print("Running: lspci & storage")
    send_ctrl_l()
    send_text("lspci\n")
    time.sleep(0.5)
    send_text("storage\n")
    time.sleep(1.0)
    capture("05_cmd_hardware.png")

    # 6. Network Subsystem
    print("Running: ifconfig & dhcp & ping gateway")
    send_ctrl_l()
    send_text("ifconfig\n")
    time.sleep(0.5)
    send_text("dhcp\n")
    time.sleep(2.0)
    send_text("ping gateway\n")
    time.sleep(4.5)
    capture("06_cmd_network.png")

    # 7. CPU Scheduling
    print("Running: sched cmp")
    send_ctrl_l()
    send_text("sched cmp\n")
    time.sleep(1.2)
    capture("07_cmd_scheduling.png")

    # 8. Memory Page Replacement
    print("Running: page cmp")
    send_ctrl_l()
    send_text("page cmp\n")
    time.sleep(1.2)
    capture("08_cmd_page_replacement.png")

    # 9. Disk Scheduling
    print("Running: disksched cmp")
    send_ctrl_l()
    send_text("disksched cmp\n")
    time.sleep(1.2)
    capture("09_cmd_disk_scheduling.png")

    # 10. Concurrency & Deadlock
    print("Running: banker detect & diners")
    send_ctrl_l()
    send_text("banker detect\n")
    time.sleep(0.8)
    send_text("diners\n")
    time.sleep(1.5)
    capture("10_cmd_concurrency.png")

    # 11. Math & Computation
    print("Running: collatz 27 & pi 100000")
    send_ctrl_l()
    send_text("collatz 27\n")
    time.sleep(0.5)
    send_text("pi 100000\n")
    time.sleep(1.2)
    capture("11_cmd_math.png")

    # 12. Errors and Warnings
    print("Running error scenarios...")
    send_ctrl_l()
    send_text("foobar\n")
    time.sleep(0.4)
    send_text("ping\n")
    time.sleep(0.4)
    send_text("dread 999999\n")
    time.sleep(0.4)
    send_text("cat nonexistent.txt\n")
    time.sleep(1.0)
    capture("12_errors_and_warnings.png")

    print("\nAll 12 screenshots captured successfully.")
    poweroff()

if __name__ == "__main__":
    main()
