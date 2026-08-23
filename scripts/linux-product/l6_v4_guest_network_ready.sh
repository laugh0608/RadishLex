#!/bin/sh
set -eu

FORMAT='radishlex-linux-l6-v4-network-ready-v1'

if [ "$#" -ne 2 ]; then
    exit 20
fi

evidence_root=$1
attempt_id=$2
script_path=$evidence_root/network-ready.sh
marker_path=$evidence_root/attempt.marker
evidence_path=$evidence_root/network.evidence
evidence_tmp=$evidence_root/.network.evidence.tmp
links_tmp=$evidence_root/.links.tmp
route4_tmp=$evidence_root/.route4.tmp
route6_tmp=$evidence_root/.route6.tmp

case "$evidence_root" in
    /var/tmp/radishlex-l6-v4-network-ready-*) ;;
    *) exit 21 ;;
esac
case "$attempt_id" in
    ''|*[!a-z0-9-]*) exit 22 ;;
esac

if [ "$(/usr/bin/id -u)" != '0' ]; then
    exit 23
fi
if [ -L "$evidence_root" ] || [ ! -d "$evidence_root" ]; then
    exit 24
fi
if [ "$(/usr/bin/stat -c '%U|%G|%a|%F' "$evidence_root")" != 'root|root|700|directory' ]; then
    exit 25
fi
if [ -L "$script_path" ] || [ ! -f "$script_path" ]; then
    exit 26
fi
if [ "$(/usr/bin/stat -c '%U|%G|%a|%h|%F' "$script_path")" != 'root|root|600|1|regular file' ]; then
    exit 27
fi
if [ -e "$marker_path" ] || [ -e "$evidence_path" ] || [ -e "$evidence_tmp" ]; then
    exit 28
fi

umask 077
if ! (set -C; : > "$marker_path") 2>/dev/null; then
    exit 29
fi
/bin/chmod 0600 "$marker_path"
/usr/bin/printf '%s\n' "$attempt_id" > "$marker_path"
/usr/bin/sync -f "$marker_path"

cleanup_private_observations() {
    /bin/rm -f -- "$links_tmp" "$route4_tmp" "$route6_tmp"
}
trap cleanup_private_observations EXIT HUP INT TERM

reason=none
set_reason() {
    if [ "$reason" = 'none' ]; then
        reason=$1
    fi
}

non_loopback_interface_count=0
for interface_path in /sys/class/net/*; do
    [ -e "$interface_path" ] || continue
    interface_name=${interface_path##*/}
    case "$interface_name" in
        ''|*[!A-Za-z0-9_.:-]*)
            set_reason interface-name-invalid
            continue
            ;;
    esac
    if [ "$interface_name" != 'lo' ]; then
        non_loopback_interface_count=$((non_loopback_interface_count + 1))
        if ! /usr/sbin/ip link set dev "$interface_name" down; then
            set_reason interface-shutdown-failed
        fi
    fi
done

if ! /usr/sbin/ip -o link show up > "$links_tmp"; then
    : > "$links_tmp"
    set_reason link-observation-failed
fi
if ! /usr/sbin/ip -4 route show table main > "$route4_tmp"; then
    : > "$route4_tmp"
    set_reason ipv4-route-observation-failed
fi
if ! /usr/sbin/ip -6 route show table main > "$route6_tmp"; then
    : > "$route6_tmp"
    set_reason ipv6-route-observation-failed
fi

active_interfaces=$(
    /usr/bin/awk -F': ' '{ name=$2; sub(/@.*/, "", name); print name }' "$links_tmp" |
        LC_ALL=C /usr/bin/sort
)
active_interface_count=$(
    /usr/bin/printf '%s\n' "$active_interfaces" |
        /usr/bin/awk 'NF { count++ } END { print count + 0 }'
)
active_interfaces_csv=$(
    /usr/bin/printf '%s\n' "$active_interfaces" |
        /usr/bin/awk 'NF { if (value != "") value=value ","; value=value $0 } END { print value }'
)
non_loopback_up_count=$(
    /usr/bin/printf '%s\n' "$active_interfaces" |
        /usr/bin/awk 'NF && $0 != "lo" { count++ } END { print count + 0 }'
)
ipv4_main_route_count=$(
    /usr/bin/awk 'NF { count++ } END { print count + 0 }' "$route4_tmp"
)
ipv6_main_route_count=$(
    /usr/bin/awk 'NF { count++ } END { print count + 0 }' "$route6_tmp"
)
boot_id=$(/usr/bin/tr -d '\n' < /proc/sys/kernel/random/boot_id)

if [ "$active_interfaces_csv" != 'lo' ] || [ "$active_interface_count" != '1' ]; then
    set_reason active-interface-mismatch
fi
if [ "$non_loopback_up_count" != '0' ]; then
    set_reason non-loopback-interface-still-up
fi
if [ "$ipv4_main_route_count" != '0' ]; then
    set_reason ipv4-main-route-not-empty
fi
if [ "$ipv6_main_route_count" != '0' ]; then
    set_reason ipv6-main-route-not-empty
fi
case "$boot_id" in
    ????????-????-????-????-????????????) ;;
    *) set_reason boot-id-invalid ;;
esac

outcome=passed
exit_code=0
if [ "$reason" != 'none' ]; then
    outcome=failed
    exit_code=10
fi

{
    /usr/bin/printf 'format=%s\n' "$FORMAT"
    /usr/bin/printf 'attempt_id=%s\n' "$attempt_id"
    /usr/bin/printf 'boot_id=%s\n' "$boot_id"
    /usr/bin/printf 'outcome=%s\n' "$outcome"
    /usr/bin/printf 'reason=%s\n' "$reason"
    /usr/bin/printf 'active_interface_count=%s\n' "$active_interface_count"
    /usr/bin/printf 'active_interfaces=%s\n' "$active_interfaces_csv"
    /usr/bin/printf 'non_loopback_interface_count=%s\n' "$non_loopback_interface_count"
    /usr/bin/printf 'non_loopback_up_count=%s\n' "$non_loopback_up_count"
    /usr/bin/printf 'ipv4_main_route_count=%s\n' "$ipv4_main_route_count"
    /usr/bin/printf 'ipv6_main_route_count=%s\n' "$ipv6_main_route_count"
} > "$evidence_tmp"
/bin/chmod 0600 "$evidence_tmp"
/usr/bin/sync -f "$evidence_tmp"
/bin/mv "$evidence_tmp" "$evidence_path"
/usr/bin/sync -f "$evidence_root"
exit "$exit_code"
