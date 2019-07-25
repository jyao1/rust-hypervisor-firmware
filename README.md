# Simple rust uefi core firmware

**This project is an experiment and should not be used production workloads.**

## Building

To compile:

cargo xbuild --release --target target.json

The result will be in:

target/target/release/hypervisor-fw

## Running

1) check out https://github.com/jyao1/edk2/tree/minovmf

2) build MinOvmf64FwPkg.

```
   build -p MinOvmf64FwPkg\MinOvmf64FwPkg.dsc -a IA32 -a X64 -t VS2015x86
```

The image is Build\MinOvmf64Fw\DEBUG_VS2015x86\fv\OVMF64Fw.fd.

3) install qemu (https://www.qemu.org/)

4) download image. (https://download.clearlinux.org/releases/28660/clear/clear-28660-kvm.img.xz)

5) run qemu

```
qemu-system-x86_64.exe -machine q35,smm=on -smp 4 -serial mon:stdio -drive if=pflash,format=raw,unit=0,file=OVMF64Fw.fd -drive if=none,id=virtio-disk0,file=clear-29160-kvm.img -device virtio-blk-pci,drive=virtio-disk0,disable-legacy=on,disable-modern=off
```

6) Then a uefi shell command prompt is shown in the command window.
It supports some simple commands.

## Boot Flow

The EDKII SEC loads hypervisor-fw as UEFI core.
Then the hypervisor-fw loads the EDKII UEFI shell.

## TODO

* implement more feature required by UEFI specification.
* remove the EDKII SEC dependency.

