on run arguments
    if (count of arguments) is not 1 then error "expected-one-shell-name"
    set shellName to item 1 of arguments
    tell application id "com.utmapp.UTM"
        set shellMachine to make new virtual machine with properties {backend:qemu, configuration:{class:qemu configuration, name:shellName, icon:"linux", notes:"RadishLex L6 upgrade_quiesced registration-only shell; never started.", architecture:"aarch64", machine:"virt", memory:4096, cpu cores:0, hypervisor:true, uefi:true, directory share mode:VirtFS, drives:{{interface:VirtIO, guest size:1024, raw:false}}, network interfaces:{}, serial ports:{}, displays:{{hardware:"virtio-gpu-pci"}}, qemu additional arguments:{}}}
        return id of shellMachine
    end tell
end run
