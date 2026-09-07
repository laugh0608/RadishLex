on run arguments
    if (count of arguments) is less than 2 then error "expected-action-and-shell-name"
    set shellAction to item 1 of arguments
    tell application id "com.utmapp.UTM"
        if shellAction is "create" then
            if (count of arguments) is not 2 then error "create-expected-shell-name"
            set shellName to item 2 of arguments
            set shellMachine to make new virtual machine with properties {backend:qemu, configuration:{name:shellName, architecture:"aarch64", drives:{{interface:VirtIO, guest size:1024, raw:false}}}}
        else if shellAction is "update" then
            if (count of arguments) is not 3 then error "update-expected-shell-id-and-name"
            set shellIdentifier to item 2 of arguments
            set shellName to item 3 of arguments
            set shellMachine to virtual machine id shellIdentifier
            if status of shellMachine is not stopped then error "registration-shell-not-stopped"
            set shellConfiguration to configuration of shellMachine
            set name of shellConfiguration to shellName
            set icon of shellConfiguration to "linux"
            set notes of shellConfiguration to "RadishLex L6 upgrade_quiesced registration-only shell; never started."
            set architecture of shellConfiguration to "aarch64"
            set machine of shellConfiguration to "virt"
            set memory of shellConfiguration to 4096
            set cpu cores of shellConfiguration to 0
            set hypervisor of shellConfiguration to true
            set uefi of shellConfiguration to true
            set directory share mode of shellConfiguration to VirtFS
            set network interfaces of shellConfiguration to {}
            set «class SrPt» of shellConfiguration to {}
            set displays of shellConfiguration to {{hardware:"virtio-gpu-pci", dynamic resolution:true}}
            set qemu additional arguments of shellConfiguration to {}
            update configuration of shellMachine with shellConfiguration
        else
            error "unsupported-registration-shell-action"
        end if
        return id of shellMachine
    end tell
end run
