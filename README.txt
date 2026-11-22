S75 LED STUDIO (RUST) - HUONG DAN NHANH
========================================

CACH 1: BUILD TRUC TIEP TREN WINDOWS (1 CLICK)
1. Giai nen thu muc nay.
2. Bam dup file `build.bat` (yeu cau may da cai Rust/Cargo).
3. Cho ~1-2 phut -> file `s75_led_studio.exe` se xuat hien ngay ben ngoai.

CACH 2: KHONG CAN CAI RUST (DUNG GITHUB ACTIONS)
1. Tao 1 repo GitHub (Public hoac Private deu duoc), upload toan bo thu muc nay len (nho giu thu muc `.github`).
2. Vao tab Actions tren GitHub -> cho ~2 phut -> tai file `s75_led_studio_windows` (da co san file .exe).

SUA GIAO THUC AULA S75 KHONG CAN BUILD LAI:
- File `protocol.json` nam cung thu muc voi `s75_led_studio.exe`.
- Sau khi bat duoc header hex tu Wireshark cua driver AULA S75, chi can sua `report_id`, `packet_header` (dang so thap phan 0-255) va `protocol_mode` trong `protocol.json`, roi bam nut "Nap lai protocol.json" ngay trong app.
