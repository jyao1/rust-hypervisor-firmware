# Simple rust uefi core firmware

**This project is an experiment and should not be used production workloads.**

## Building

1) Install rust (https://www.rust-lang.org/)

2) Intall xbuild

```
cargo install cargo-xbuild
```

3) Compile the code

```
cargo xbuild --release --target target.json
```

4) The result will be in:

target/target/release/payload-efi

5) Compiler grub (option)

Download grub
```
https://github.com/rhboot/grub2.git 
```

Build grub
```
../configure --prefix /home/luxy/local --host=x86_64-linux-gnu --target=x86_64-linux-gnu --with-platform=efi --enable-boot-time --enable-mm-debug --enable-cache-stats 
```

Install grub
```
sudo losetup -f -P clear-31380-kvm.img 
sudo mount /dev/loop1p1 /mnt/clear-kvm-img/ 
sudo ./grub-install --efi-directory /mnt/clear-kvm-img --target x86_64-efi 
```

## Running

1) check out https://github.com/jyao1/edk2/tree/minovmf

2) build MinOvmf64FwPkg.

```
   build -p MinOvmf64FwPkg\MinOvmf64RustFwPkg.dsc -a IA32 -a X64 -t CLANGPDB
```

The image is Build\MinOvmf64Fw\DEBUG_CLANGPDB\fv\OVMF64RUSTFW.fd.

3) install qemu (https://www.qemu.org/)

4) download image. (https://download.clearlinux.org/releases/31380/clear/clear-31380-kvm.img.xz)

5) run qemu

```
qemu-system-x86_64.exe -machine q35,smm=on -smp 4 -serial mon:stdio -drive if=pflash,format=raw,unit=0,file=OVMF64RUSTFW.fd -drive if=none,id=virtio-disk0,file=clear-31380-kvm.img -device virtio-blk-pci,drive=virtio-disk0,disable-legacy=on,disable-modern=off  -vnc 0.0.0.0:1 -m 1g --enable-kvm 
```

6) Then a uefi shell command prompt is shown in the command window.
It supports some simple commands, such as memmap.

```
EFI\BOOT\BOOTX64.efi
```

7) It will boot to grub.

Add kernel information:
```
linux (hd0,gpt1)/EFI/org.clearlinux/kernel-org.clearlinux.kvm.5.3.7-396 root=PARTUUID=492838b1-9d22-484a-a59f-fdd6f18d188c quiet console=hvc0 console=tty0 console=ttyS0,115200n8 cryptomgr.notests init=/usr/lib/systemd/systemd-bootchart initcall_debug no_timer_check noreplace-smp page_alloc.shuffle=1 rootfstype=ext4,btrfs,xfs tsc=reliable rw 
```

Then

```
boot
```

8) It will boot to kernel. You can see the last debug message is exit_boot_service.

## Boot Flow

The payload-efi can be treated as a UEFI payload.

The EDKII SEC loads payload-efi as UEFI core.

Then the payload-efi loads the EDKII UEFI shell.

## TODO

* implement more feature required by UEFI specification.
* remove the EDKII SEC dependency.

