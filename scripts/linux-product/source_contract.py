from __future__ import annotations

import hashlib
import re
from pathlib import Path
from typing import Any

from product_metadata import (
    REPO_ROOT,
    LinuxProductMetadata,
    LinuxProductMetadataError,
    expected_addon_metadata,
    expected_input_method_metadata,
    load_json_object,
    load_layout,
    product_data,
    read_text,
    render_control,
    validate_desktop_entry,
    validate_icon,
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def required_match(pattern: str, text: str, label: str) -> str:
    match = re.search(pattern, text, re.MULTILINE)
    if match is None:
        raise LinuxProductMetadataError(f"cannot resolve {label}")
    return match.group(1)


def validate_source_contract(
    metadata: LinuxProductMetadata | None = None,
    repo_root: Path = REPO_ROOT,
) -> tuple[LinuxProductMetadata, dict[str, Any]]:
    if metadata is None:
        metadata = LinuxProductMetadata.load(repo_root / "packaging/linux/product.json")
    layout = load_layout(metadata, repo_root / "packaging/linux/install-layout.json")

    version = load_json_object(repo_root / "version.json", "version.json")
    expected_version = {
        "schemaVersion": 1,
        "productVersion": metadata.product_version,
        "flutterBuildNumber": int(metadata.build_number),
    }
    if version != expected_version:
        raise LinuxProductMetadataError(
            "version.json differs from Linux product metadata"
        )

    pubspec = read_text(
        repo_root / "apps/radishlex-manager/pubspec.yaml", "Manager pubspec"
    )
    version_match = re.search(
        r"^version:\s*([^\s+]+)\+([^\s]+)\s*$", pubspec, re.MULTILINE
    )
    if version_match is None or version_match.groups() != (
        metadata.product_version,
        metadata.build_number,
    ):
        raise LinuxProductMetadataError(
            "Manager version differs from Linux product metadata"
        )
    if not re.search(r"^\s*uses-material-design:\s*true\s*$", pubspec, re.MULTILINE):
        raise LinuxProductMetadataError(
            "Manager must declare the Material Icons generated asset"
        )

    manager_cmake = read_text(
        repo_root / "apps/radishlex-manager/linux/CMakeLists.txt",
        "Manager Linux CMake",
    )
    binary_name = required_match(
        r'^set\(BINARY_NAME\s+"([^"]+)"\)$', manager_cmake, "Manager binary name"
    )
    application_id = required_match(
        r'^set\(APPLICATION_ID\s+"([^"]+)"\)$',
        manager_cmake,
        "Manager application ID",
    )
    if binary_name != "radishlex_manager":
        raise LinuxProductMetadataError("Manager Linux binary name is not fixed")
    if application_id != metadata.manager_application_id:
        raise LinuxProductMetadataError(
            "Manager Linux application ID differs from product metadata"
        )

    contract = read_text(
        repo_root / "crates/ime-ffi/src/contract.rs", "Rust FFI contract"
    )
    header = read_text(
        repo_root / "crates/ime-ffi/include/radishlex_input.h", "C FFI header"
    )
    dart = read_text(
        repo_root
        / "apps/radishlex-manager/lib/src/bridge/ffi_dynamic_native_api.dart",
        "Dart FFI contract",
    )
    abi_versions = {
        int(
            required_match(
                r"RADISHLEX_ABI_CONTRACT_VERSION:\s*u32\s*=\s*([0-9]+)",
                contract,
                "Rust FFI ABI",
            )
        ),
        int(
            required_match(
                r"^#define RADISHLEX_ABI_CONTRACT_VERSION\s+([0-9]+)u$",
                header,
                "C FFI ABI",
            )
        ),
        int(
            required_match(
                r"_expectedContractVersion\s*=\s*([0-9]+);",
                dart,
                "Dart FFI ABI",
            )
        ),
    }
    if abi_versions != {metadata.ffi_abi_version}:
        raise LinuxProductMetadataError(
            "FFI ABI declarations differ from Linux product metadata"
        )

    userdb = read_text(
        repo_root / "crates/ime-userdb/src/store/connection.rs",
        "userdb connection source",
    )
    userdb_schema = int(
        required_match(
            r"SCHEMA_VERSION:\s*i64\s*=\s*([0-9]+)",
            userdb,
            "userdb schema",
        )
    )
    if userdb_schema != metadata.userdb_schema_version:
        raise LinuxProductMetadataError(
            "userdb schema differs from Linux product metadata"
        )

    settings = read_text(
        repo_root
        / "apps/radishlex-manager/lib/src/bridge/manager_settings_store.dart",
        "Manager settings store",
    )
    if f"if (formatVersion != {metadata.settings_format_version})" not in settings:
        raise LinuxProductMetadataError(
            "Manager settings format differs from Linux product metadata"
        )
    privacy = read_text(
        repo_root / "platforms/linux-fcitx5/src/privacy_mode.cpp",
        "Linux privacy source",
    )
    expected_privacy_json = (
        f'"{{\\n  \\"format_version\\": '
        f'{metadata.privacy_format_version},\\n  \\"privacy_mode\\": "'
    )
    if "void versionOne()" not in privacy or expected_privacy_json not in privacy:
        raise LinuxProductMetadataError(
            "Linux privacy format differs from product metadata"
        )

    app_source = read_text(
        repo_root / "apps/radishlex-manager/lib/src/app.dart", "Manager theme"
    )
    dejavu_index = app_source.find("'DejaVu Sans'")
    noto_index = app_source.find("'Noto Sans CJK SC'")
    if dejavu_index < 0 or noto_index < 0 or dejavu_index >= noto_index:
        raise LinuxProductMetadataError(
            "Manager text font fallback differs from the Debian font profile"
        )

    platform_cmake = read_text(
        repo_root / "platforms/linux-fcitx5/CMakeLists.txt", "Fcitx5 CMake"
    )
    required_cmake_phrases = (
        'RADISHLEX_PRODUCT_VERSION "0.1.0"',
        'RADISHLEX_RUNTIME_LAYOUT_PROFILE "staged"',
        'RADISHLEX_SYSTEM_RIME_DATA_DIR="/usr/share/radishlex/rime"',
        'RADISHLEX_RUNTIME_LAYOUT_PROFILE STREQUAL "system"',
    )
    for phrase in required_cmake_phrases:
        if phrase not in platform_cmake:
            raise LinuxProductMetadataError(
                f"Fcitx5 CMake is missing product-profile contract: {phrase}"
            )
    addon_template = read_text(
        repo_root / "platforms/linux-fcitx5/config/radishlex-addon.conf.in",
        "Fcitx addon metadata template",
    )
    if addon_template.replace(
        "@PROJECT_VERSION@", metadata.product_version
    ) != expected_addon_metadata(metadata):
        raise LinuxProductMetadataError(
            "Fcitx addon metadata does not consume the product build version"
        )
    input_method_metadata = read_text(
        repo_root / "platforms/linux-fcitx5/config/radishlex.conf.in",
        "Fcitx input-method metadata",
    )
    if input_method_metadata != expected_input_method_metadata():
        raise LinuxProductMetadataError(
            "Fcitx input-method metadata differs from the product contract"
        )
    runtime_layout = read_text(
        repo_root / "platforms/linux-fcitx5/src/runtime_layout.cpp",
        "Linux runtime layout source",
    )
    if (
        "RADISHLEX_SYSTEM_RIME_DATA_DIR" not in runtime_layout
        or 'kRimeDataDirectory = "radishlex-rime"' not in runtime_layout
    ):
        raise LinuxProductMetadataError(
            "Linux runtime layout does not separate staged and system profiles"
        )

    manager_builder = read_text(
        repo_root / "scripts/build-manager-linux-product.sh",
        "Linux Manager product builder",
    )
    addon_builder = read_text(
        repo_root / "scripts/build-linux-product-addon-stage.sh",
        "Linux product addon builder",
    )
    layout_gate = read_text(
        repo_root / "scripts/check-linux-product-layout.sh",
        "Linux product layout gate",
    )
    required_scripts = {
        "Linux Manager product builder": (
            manager_builder,
            (
                "CARGO_ENCODED_RUSTFLAGS",
                "--remap-path-prefix=${repo_root}=",
                "--remap-path-prefix=${cargo_home}=",
                "env -u RUSTFLAGS",
            ),
        ),
        "Linux product addon builder": (
            addon_builder,
            (
                "field product_version",
                "-ffile-prefix-map=${repo_root}=",
                "-ffile-prefix-map=${temp_dir}=",
                "-DRADISHLEX_RUNTIME_LAYOUT_PROFILE=system",
                'DESTDIR="${stage_dir}" cmake --install',
                "validate-addon-stage --addon-stage",
            ),
        ),
        "Linux product layout gate": (
            layout_gate,
            (
                "umask 022",
                "--manager-bundle",
                "--addon-stage",
                '"${repo_root}/scripts/linux-product/rootfs.py" assemble',
                'rg -F "${HOME}/"',
                "fc-match",
                "nm -D --defined-only",
            ),
        ),
    }
    for label, (script, phrases) in required_scripts.items():
        for phrase in phrases:
            if phrase not in script:
                raise LinuxProductMetadataError(
                    f"{label} is missing contract phrase: {phrase}"
                )
        if re.search(
            r"^[ \t]*(?:sudo|apt|apt-get|dpkg|systemctl|service|fcitx5)\b",
            script,
            re.MULTILINE,
        ):
            raise LinuxProductMetadataError(
                f"{label} must not mutate packages, services, or sessions"
            )

    rime_lock_path = repo_root / "packaging/rime/product-rime-data.json"
    rime_lock = product_data.validate_source(
        rime_lock_path,
        repo_root / "packaging/rime",
    )
    if sha256(rime_lock_path) != metadata.rime_data_lock_sha256:
        raise LinuxProductMetadataError(
            "RimeData lock hash differs from Linux product metadata"
        )
    if rime_lock["format_version"] != metadata.rime_data_manifest_version:
        raise LinuxProductMetadataError(
            "RimeData manifest format differs from Linux product metadata"
        )
    if rime_lock["schema_id"] != metadata.rime_schema_id:
        raise LinuxProductMetadataError(
            "Rime schema differs from Linux product metadata"
        )

    render_control(metadata, repo_root / "packaging/linux/debian/control.in")
    validate_desktop_entry(
        metadata,
        layout,
        repo_root
        / "packaging/linux/assets/dev.radishlex.radishlexManager.desktop",
    )
    validate_icon(repo_root / "packaging/linux/assets/radishlex.svg")
    license_path = repo_root / "LICENSE"
    if (
        license_path.is_symlink()
        or not license_path.is_file()
        or license_path.stat().st_size == 0
    ):
        raise LinuxProductMetadataError(
            "repository product license must be a non-empty regular file"
        )
    return metadata, layout
