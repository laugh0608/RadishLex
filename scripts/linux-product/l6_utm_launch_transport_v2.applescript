on run arguments
    if (count of arguments) is not 1 then error "expected-one-target-uuid"
    set targetIdentifier to item 1 of arguments
    tell application id "com.utmapp.UTM"
        activate
        set targetMachine to virtual machine id targetIdentifier
        start targetMachine saving true recovery false
    end tell
end run
