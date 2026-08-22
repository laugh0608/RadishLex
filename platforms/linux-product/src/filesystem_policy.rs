pub(crate) const fn parent_permissions_are_safe(mode: u32, allow_shared_lock_parent: bool) -> bool {
    if allow_shared_lock_parent {
        // Debian exposes /run/lock as root-owned 01777. The sticky bit prevents
        // other users from replacing a root-owned 0600 guard; a pre-created
        // non-root entry is still rejected by the guard identity checks.
        mode & 0o002 == 0 || mode == 0o1777
    } else {
        mode & 0o022 == 0
    }
}
