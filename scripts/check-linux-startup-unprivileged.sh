#!/usr/bin/env bash
# Run just the two-UID regression using existing local artifacts.
set -euo pipefail

if [[ $# != 2 || ! -x "$2" ]]; then
  echo "Usage: $0 LOCAL_DEBIAN_IMAGE LINUX_TEST_EXECUTABLE" >&2
  exit 2
fi

image="$1"
binary="$(cd "$(dirname "$2")" && pwd -P)/$(basename "$2")"
test_name="startup::tests::unprivileged::both_components_in_a_non_root_process"
mounts=()
for path in /var/lib/radishlex /var/lib/dpkg /usr/share \
    /usr/lib/aarch64-linux-gnu/fcitx5 /usr/lib/aarch64-linux-gnu/radishlex; do
  mounts+=(--tmpfs "$path:rw,nosuid,nodev,size=4m,mode=0755")
done
# libtest accepts a missing filter with exit 0. Require this exact test to exist.
docker_args=(run --rm --pull=never --network=none --read-only --cpus=1 --memory=384m
  --pids-limit=48 --cap-drop=ALL --cap-add=SETUID --cap-add=SETGID
  --security-opt=no-new-privileges --tmpfs /tmp:rw,nosuid,nodev,size=64m,mode=1777
  --tmpfs /run/lock:rw,nosuid,nodev,size=1m,mode=1777 "${mounts[@]}"
  --mount "type=bind,src=$binary,dst=/probe,readonly" --entrypoint /probe "$image")
listing="$(docker "${docker_args[@]}" --list --ignored --exact "$test_name")"
if ! grep -Fxq "$test_name: test" <<<"$listing"; then
  echo "Expected Linux startup regression is missing from the executable" >&2
  exit 1
fi
docker "${docker_args[@]}" --ignored --exact "$test_name" --nocapture
