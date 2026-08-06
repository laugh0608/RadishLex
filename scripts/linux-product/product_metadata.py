#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_ROOT = REPO_ROOT / "packaging/linux"
METADATA_PATH = PACKAGE_ROOT / "product.json"
LAYOUT_PATH = PACKAGE_ROOT / "install-layout.json"
CONTROL_TEMPLATE_PATH = PACKAGE_ROOT / "debian/control.in"
DESKTOP_ENTRY_PATH = (
    PACKAGE_ROOT / "assets/dev.radishlex.radishlexManager.desktop"
)
ICON_PATH = PACKAGE_ROOT / "assets/radishlex.svg"
RIME_SCRIPT_DIR = REPO_ROOT / "scripts/rime-product"
if str(RIME_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(RIME_SCRIPT_DIR))
import product_data  # noqa: E402

if __name__ == "__main__":
    # Keep the lazily imported source contract bound to this CLI module.
    sys.modules.setdefault("product_metadata", sys.modules[__name__])


METADATA_KEYS = {
    "format_version",
    "product_manifest_format_version",
    "install_layout_format_version",
    "product_id",
    "product_version",
    "build_number",
    "distribution_identity",
    "debian_release",
    "debian_codename",
    "package_name",
    "debian_revision",
    "package_version",
    "debian_architecture",
    "multiarch_tuple",
    "runtime_layout",
    "data_layout",
    "settings_format_version",
    "privacy_format_version",
    "ffi_abi_version",
    "userdb_schema_version",
    "rime_data_manifest_version",
    "rime_data_lock_sha256",
    "rime_schema_id",
    "manager_application_id",
    "fcitx_minimum_version",
    "librime_package",
    "librime_minimum_version",
    "hard_dependencies",
    "default_removal",
    "data_removal",
}
INTEGER_FIELDS = {
    "format_version",
    "product_manifest_format_version",
    "install_layout_format_version",
    "settings_format_version",
    "privacy_format_version",
    "ffi_abi_version",
    "userdb_schema_version",
    "rime_data_manifest_version",
}
STRING_FIELDS = METADATA_KEYS - INTEGER_FIELDS - {"hard_dependencies"}
CALVER_PATTERN = re.compile(r"[0-9]{2}\.(?:[1-9]|1[0-2])\.[1-9][0-9]*")
VERSION_PATTERN = re.compile(r"[0-9]+(?:\.[0-9]+){1,3}")
IDENTIFIER_PATTERN = re.compile(r"[A-Za-z0-9][A-Za-z0-9.-]*")
LOWER_IDENTIFIER_PATTERN = re.compile(r"[a-z0-9][a-z0-9-]*")
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
EXPECTED_FORBIDDEN_PREFIXES = [
    "/etc",
    "/home",
    "/opt",
    "/root",
    "/run",
    "/tmp",
    "/usr/lib/systemd",
    "/usr/local",
    "/usr/share/autostart",
    "/usr/share/xdg/autostart",
    "/var",
]
EXPECTED_FONT_SUFFIXES = [".otf", ".ttc", ".ttf"]


class LinuxProductMetadataError(RuntimeError):
    pass


@dataclass(frozen=True)
class LinuxProductMetadata:
    format_version: int
    product_manifest_format_version: int
    install_layout_format_version: int
    product_id: str
    product_version: str
    build_number: str
    distribution_identity: str
    debian_release: str
    debian_codename: str
    package_name: str
    debian_revision: str
    package_version: str
    debian_architecture: str
    multiarch_tuple: str
    runtime_layout: str
    data_layout: str
    settings_format_version: int
    privacy_format_version: int
    ffi_abi_version: int
    userdb_schema_version: int
    rime_data_manifest_version: int
    rime_data_lock_sha256: str
    rime_schema_id: str
    manager_application_id: str
    fcitx_minimum_version: str
    librime_package: str
    librime_minimum_version: str
    hard_dependencies: tuple[str, ...]
    default_removal: str
    data_removal: str

    @classmethod
    def load(cls, path: Path = METADATA_PATH) -> "LinuxProductMetadata":
        value = load_json_object(path, "Linux product metadata")
        if set(value) != METADATA_KEYS:
            raise LinuxProductMetadataError(
                "Linux product metadata fields do not match format v1"
            )
        for field in INTEGER_FIELDS:
            require_positive_integer(value[field], field)
        for field in STRING_FIELDS:
            require_string(value[field], field)
        dependencies = value["hard_dependencies"]
        if not isinstance(dependencies, list) or not dependencies:
            raise LinuxProductMetadataError(
                "hard_dependencies must be a non-empty list"
            )
        for dependency in dependencies:
            require_string(dependency, "hard_dependencies entry")
        if len(set(dependencies)) != len(dependencies):
            raise LinuxProductMetadataError("hard_dependencies must be unique")

        metadata = cls(
            **{
                **value,
                "hard_dependencies": tuple(dependencies),
            }
        )
        metadata.validate()
        return metadata

    def validate(self) -> None:
        if self.format_version != 1:
            raise LinuxProductMetadataError("unsupported Linux metadata format")
        for field in (
            "product_manifest_format_version",
            "install_layout_format_version",
            "settings_format_version",
            "privacy_format_version",
            "rime_data_manifest_version",
        ):
            if getattr(self, field) != 1:
                raise LinuxProductMetadataError(f"{field} must remain version 1")
        if CALVER_PATTERN.fullmatch(self.product_version) is None:
            raise LinuxProductMetadataError(
                "product_version must use Radish YY.M.RELEASE CalVer"
            )
        if not self.build_number.isdigit() or int(self.build_number) <= 0:
            raise LinuxProductMetadataError(
                "build_number must be a positive decimal string"
            )
        if not self.debian_revision.isdigit() or int(self.debian_revision) <= 0:
            raise LinuxProductMetadataError(
                "debian_revision must be a positive decimal string"
            )
        if VERSION_PATTERN.fullmatch(self.fcitx_minimum_version) is None:
            raise LinuxProductMetadataError("fcitx_minimum_version is invalid")
        if VERSION_PATTERN.fullmatch(self.librime_minimum_version) is None:
            raise LinuxProductMetadataError("librime_minimum_version is invalid")
        if SHA256_PATTERN.fullmatch(self.rime_data_lock_sha256) is None:
            raise LinuxProductMetadataError(
                "rime_data_lock_sha256 must be a lowercase SHA-256"
            )
        if LOWER_IDENTIFIER_PATTERN.fullmatch(self.product_id) is None:
            raise LinuxProductMetadataError("product_id is invalid")
        if LOWER_IDENTIFIER_PATTERN.fullmatch(self.package_name) is None:
            raise LinuxProductMetadataError("package_name is invalid")
        if IDENTIFIER_PATTERN.fullmatch(self.manager_application_id) is None:
            raise LinuxProductMetadataError("manager_application_id is invalid")

        exact_values = {
            "product_id": "radishlex-linux",
            "distribution_identity": "debian-local-deb-v1",
            "debian_release": "13",
            "debian_codename": "trixie",
            "package_name": "radishlex",
            "debian_architecture": "arm64",
            "multiarch_tuple": "aarch64-linux-gnu",
            "runtime_layout": "debian-system-v1",
            "data_layout": "xdg-v1",
            "rime_schema_id": "radishlex_pinyin",
            "manager_application_id": "dev.radishlex.radishlexManager",
            "librime_package": "librime1t64",
            "default_removal": "programs-only",
            "data_removal": "separate-authorized-flow",
        }
        for field, expected in exact_values.items():
            if getattr(self, field) != expected:
                raise LinuxProductMetadataError(
                    f"{field} must remain {expected} for this distribution identity"
                )

        expected_package_version = (
            f"{self.product_version}+{self.build_number}-{self.debian_revision}"
        )
        if self.package_version != expected_package_version:
            raise LinuxProductMetadataError(
                "package_version differs from product/build/revision"
            )
        expected_dependencies = (
            "${shlibs:Depends}",
            "${misc:Depends}",
            f"fcitx5 (>= {self.fcitx_minimum_version})",
            f"{self.librime_package} (>= {self.librime_minimum_version})",
            "fonts-dejavu-core",
            "fonts-noto-cjk",
        )
        if self.hard_dependencies != expected_dependencies:
            raise LinuxProductMetadataError(
                "hard_dependencies differ from the Debian 13 product profile"
            )

    def manifest_fields(self) -> dict[str, Any]:
        return {
            "build_number": self.build_number,
            "data_layout": self.data_layout,
            "data_removal": self.data_removal,
            "debian_architecture": self.debian_architecture,
            "debian_codename": self.debian_codename,
            "debian_release": self.debian_release,
            "default_removal": self.default_removal,
            "distribution_identity": self.distribution_identity,
            "ffi_abi_version": self.ffi_abi_version,
            "hard_dependencies": list(self.hard_dependencies),
            "install_layout_format_version": self.install_layout_format_version,
            "manager_application_id": self.manager_application_id,
            "multiarch_tuple": self.multiarch_tuple,
            "package_name": self.package_name,
            "package_version": self.package_version,
            "privacy_format_version": self.privacy_format_version,
            "product_id": self.product_id,
            "product_manifest_format_version": self.product_manifest_format_version,
            "product_version": self.product_version,
            "rime_data_manifest_version": self.rime_data_manifest_version,
            "rime_data_lock_sha256": self.rime_data_lock_sha256,
            "rime_schema_id": self.rime_schema_id,
            "runtime_layout": self.runtime_layout,
            "settings_format_version": self.settings_format_version,
            "userdb_schema_version": self.userdb_schema_version,
        }


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise LinuxProductMetadataError(f"invalid {label}: {exc}") from exc
    if not isinstance(value, dict):
        raise LinuxProductMetadataError(f"{label} must contain an object")
    return value


def require_positive_integer(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise LinuxProductMetadataError(f"{field} must be a positive integer")
    return value


def require_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value or value != value.strip():
        raise LinuxProductMetadataError(
            f"{field} must be a non-empty normalized string"
        )
    return value


def absolute_system_path(value: Any, field: str) -> str:
    text = require_string(value, field)
    path = PurePosixPath(text)
    if (
        not path.is_absolute()
        or path.as_posix() != text
        or "//" in text
        or any(part in ("", ".", "..") for part in path.parts[1:])
    ):
        raise LinuxProductMetadataError(
            f"{field} must be an absolute normalized system path"
        )
    return text


def expected_paths(metadata: LinuxProductMetadata) -> dict[str, str]:
    library_root = f"/usr/lib/{metadata.multiarch_tuple}"
    manager_root = f"{library_root}/radishlex/manager"
    application_id = metadata.manager_application_id
    return {
        "manager_root": manager_root,
        "manager_executable": f"{manager_root}/radishlex_manager",
        "manager_ffi": f"{manager_root}/lib/libradishlex_ime_ffi.so",
        "addon_library": f"{library_root}/fcitx5/radishlex.so",
        "addon_ffi": f"{library_root}/fcitx5/libradishlex_ime_ffi.so",
        "rime_data_root": "/usr/share/radishlex/rime",
        "product_manifest": "/usr/share/radishlex/product-manifest.json",
        "addon_metadata": "/usr/share/fcitx5/addon/radishlex.conf",
        "input_method_metadata": "/usr/share/fcitx5/inputmethod/radishlex.conf",
        "desktop_entry": f"/usr/share/applications/{application_id}.desktop",
        "manager_icon": (
            f"/usr/share/icons/hicolor/scalable/apps/{application_id}.svg"
        ),
        "input_method_icon": (
            "/usr/share/icons/hicolor/scalable/apps/fcitx-radishlex.svg"
        ),
        "copyright": "/usr/share/doc/radishlex/copyright",
    }


def expected_components(metadata: LinuxProductMetadata) -> list[dict[str, str]]:
    paths = expected_paths(metadata)
    addon_root = f"usr/lib/{metadata.multiarch_tuple}/fcitx5"
    return [
        {
            "component_id": "manager-bundle",
            "source_id": "manager_bundle",
            "source_path": ".",
            "target_path": paths["manager_root"],
            "kind": "tree",
        },
        {
            "component_id": "fcitx-addon",
            "source_id": "addon_stage",
            "source_path": f"{addon_root}/radishlex.so",
            "target_path": paths["addon_library"],
            "kind": "file",
        },
        {
            "component_id": "fcitx-ffi",
            "source_id": "addon_stage",
            "source_path": f"{addon_root}/libradishlex_ime_ffi.so",
            "target_path": paths["addon_ffi"],
            "kind": "file",
        },
        {
            "component_id": "rime-data",
            "source_id": "product_rime_data",
            "source_path": ".",
            "target_path": paths["rime_data_root"],
            "kind": "tree",
        },
        {
            "component_id": "fcitx-addon-metadata",
            "source_id": "addon_stage",
            "source_path": "usr/share/fcitx5/addon/radishlex.conf",
            "target_path": paths["addon_metadata"],
            "kind": "file",
        },
        {
            "component_id": "fcitx-input-method-metadata",
            "source_id": "addon_stage",
            "source_path": "usr/share/fcitx5/inputmethod/radishlex.conf",
            "target_path": paths["input_method_metadata"],
            "kind": "file",
        },
        {
            "component_id": "manager-desktop-entry",
            "source_id": "linux_packaging",
            "source_path": "assets/dev.radishlex.radishlexManager.desktop",
            "target_path": paths["desktop_entry"],
            "kind": "file",
        },
        {
            "component_id": "manager-icon",
            "source_id": "linux_packaging",
            "source_path": "assets/radishlex.svg",
            "target_path": paths["manager_icon"],
            "kind": "file",
        },
        {
            "component_id": "input-method-icon",
            "source_id": "linux_packaging",
            "source_path": "assets/radishlex.svg",
            "target_path": paths["input_method_icon"],
            "kind": "file",
        },
        {
            "component_id": "product-license",
            "source_id": "repository",
            "source_path": "LICENSE",
            "target_path": paths["copyright"],
            "kind": "file",
        },
    ]


def expected_layout(metadata: LinuxProductMetadata) -> dict[str, Any]:
    paths = expected_paths(metadata)
    return {
        "format_version": metadata.install_layout_format_version,
        "layout_id": metadata.runtime_layout,
        "installation_scope": "system",
        "owner": {"uid": 0, "gid": 0},
        "modes": {
            "directory": "0755",
            "executable": "0755",
            "regular_file": "0644",
        },
        "paths": paths,
        "components": expected_components(metadata),
        "ffi_equivalence": {
            "relationship": "same-content-distinct-inode",
            "paths": [paths["manager_ffi"], paths["addon_ffi"]],
        },
        "forbidden_prefixes": EXPECTED_FORBIDDEN_PREFIXES,
        "forbidden_font_suffixes": EXPECTED_FONT_SUFFIXES,
        "allowed_embedded_font_paths": [
            f"{paths['manager_root']}/data/flutter_assets/fonts/"
            "MaterialIcons-Regular.otf"
        ],
        "default_removal": metadata.default_removal,
        "data_removal": metadata.data_removal,
    }


def load_layout(
    metadata: LinuxProductMetadata, path: Path = LAYOUT_PATH
) -> dict[str, Any]:
    value = load_json_object(path, "Linux install layout")
    expected = expected_layout(metadata)
    if value != expected:
        raise LinuxProductMetadataError(
            "Linux install layout differs from debian-system-v1"
        )
    for name, system_path in value["paths"].items():
        absolute_system_path(system_path, f"paths.{name}")
    for index, component in enumerate(value["components"]):
        absolute_system_path(
            component["target_path"], f"components[{index}].target_path"
        )
    return value


def expected_control_template() -> str:
    return (
        "Package: @PACKAGE_NAME@\n"
        "Version: @PACKAGE_VERSION@\n"
        "Architecture: @DEBIAN_ARCHITECTURE@\n"
        "Maintainer: RadishLex <laugh0608@foxmail.com>\n"
        "Section: utils\n"
        "Priority: optional\n"
        "Depends: @HARD_DEPENDENCIES@\n"
        "Homepage: https://github.com/laugh0608/RadishLex\n"
        "Description: Local-first Chinese input system for Fcitx5\n"
        " RadishLex combines a native Fcitx5 addon with a local management "
        "application.\n"
        " This template describes the Debian 13 ARM64 local acceptance "
        "carrier only.\n"
    )


def render_control(
    metadata: LinuxProductMetadata,
    template_path: Path = CONTROL_TEMPLATE_PATH,
) -> str:
    template = read_text(template_path, "Debian control template")
    if template != expected_control_template():
        raise LinuxProductMetadataError(
            "Debian control template differs from the local carrier contract"
        )
    rendered = template
    replacements = {
        "@PACKAGE_NAME@": metadata.package_name,
        "@PACKAGE_VERSION@": metadata.package_version,
        "@DEBIAN_ARCHITECTURE@": metadata.debian_architecture,
        "@HARD_DEPENDENCIES@": ", ".join(metadata.hard_dependencies),
    }
    for token, replacement in replacements.items():
        rendered = rendered.replace(token, replacement)
    if (
        re.search(r"@[A-Z_]+@", rendered) is not None
        or "Recommends:" in rendered
        or "Suggests:" in rendered
    ):
        raise LinuxProductMetadataError("Debian control rendering is incomplete")
    return rendered


def render_shlibdeps_control(metadata: LinuxProductMetadata) -> str:
    return (
        f"Source: {metadata.package_name}\n"
        "Section: utils\n"
        "Priority: optional\n"
        "Maintainer: RadishLex <laugh0608@foxmail.com>\n"
        "Standards-Version: 4.7.2.0\n"
        "Rules-Requires-Root: no\n"
        "\n"
        f"Package: {metadata.package_name}\n"
        f"Architecture: {metadata.debian_architecture}\n"
        "Description: Dependency analysis view for the local acceptance carrier\n"
        " This ephemeral control view is used only by dpkg-shlibdeps.\n"
    )


def expected_desktop_entry(
    metadata: LinuxProductMetadata, layout: dict[str, Any]
) -> str:
    executable = layout["paths"]["manager_executable"]
    return (
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Version=1.0\n"
        "Name=RadishLex Manager\n"
        "Name[zh_CN]=萝卜词核管理器\n"
        "Comment=Manage local RadishLex dictionaries, learning and privacy\n"
        "Comment[zh_CN]=管理萝卜词核本地词库、学习与隐私设置\n"
        f"Exec={executable}\n"
        f"TryExec={executable}\n"
        f"Icon={metadata.manager_application_id}\n"
        "Terminal=false\n"
        "Categories=Settings;Utility;\n"
        "StartupNotify=true\n"
    )


def expected_addon_metadata(metadata: LinuxProductMetadata) -> str:
    return (
        "[Addon]\n"
        "Name=RadishLex\n"
        "Name[zh_CN]=萝卜词核\n"
        "Category=InputMethod\n"
        f"Version={metadata.product_version}\n"
        "Library=radishlex\n"
        "Type=SharedLibrary\n"
        "OnDemand=True\n"
        "Configurable=False\n"
    )


def expected_input_method_metadata() -> str:
    return (
        "[InputMethod]\n"
        "Name=RadishLex Pinyin\n"
        "Name[zh_CN]=萝卜词核拼音\n"
        "Icon=fcitx-radishlex\n"
        "Label=萝\n"
        "LangCode=zh_CN\n"
        "Addon=radishlex\n"
        "Configurable=False\n"
    )


def validate_desktop_entry(
    metadata: LinuxProductMetadata,
    layout: dict[str, Any],
    path: Path = DESKTOP_ENTRY_PATH,
) -> None:
    value = read_text(path, "Linux desktop entry")
    if value != expected_desktop_entry(metadata, layout):
        raise LinuxProductMetadataError(
            "Linux desktop entry differs from the fixed Manager launch contract"
        )
    forbidden = ("sh -c", "bash -c", "env ", "$", "%", "LD_LIBRARY_PATH")
    if any(token in value for token in forbidden):
        raise LinuxProductMetadataError(
            "Linux desktop entry contains a shell or environment indirection"
        )


def validate_icon(path: Path = ICON_PATH) -> None:
    if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
        raise LinuxProductMetadataError("Linux icon must be a non-empty regular file")
    try:
        root = ET.fromstring(path.read_bytes())
    except Exception as exc:
        raise LinuxProductMetadataError(f"invalid Linux SVG icon: {exc}") from exc
    if root.tag != "{http://www.w3.org/2000/svg}svg":
        raise LinuxProductMetadataError("Linux icon root must be SVG")
    if root.attrib != {"viewBox": "0 0 64 64"}:
        raise LinuxProductMetadataError("Linux icon viewBox is not fixed")
    children = list(root)
    if len(children) != 3:
        raise LinuxProductMetadataError("Linux icon must contain three paths")
    for child in children:
        if child.tag != "{http://www.w3.org/2000/svg}path":
            raise LinuxProductMetadataError("Linux icon contains an unsupported node")
        if set(child.attrib) != {"fill", "d"} or not child.attrib["d"]:
            raise LinuxProductMetadataError("Linux icon path is incomplete")
    lower = path.read_text(encoding="utf-8").lower()
    for forbidden in ("<script", "href=", "xlink:", "data:"):
        if forbidden in lower:
            raise LinuxProductMetadataError(
                "Linux icon contains scripting or an external resource"
            )


def read_text(path: Path, label: str) -> str:
    try:
        value = path.read_text(encoding="utf-8")
    except Exception as exc:
        raise LinuxProductMetadataError(f"cannot read {label}: {exc}") from exc
    if not value.endswith("\n"):
        raise LinuxProductMetadataError(f"{label} must end with a newline")
    return value


def validate_source_contract(
    metadata: LinuxProductMetadata | None = None,
    repo_root: Path = REPO_ROOT,
) -> tuple[LinuxProductMetadata, dict[str, Any]]:
    from source_contract import validate_source_contract as validate

    return validate(metadata, repo_root)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate RadishLex Linux product metadata."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate-source")
    field = subparsers.add_parser("field")
    field.add_argument(
        "name",
        choices=(
            "debian_architecture",
            "librime_package",
            "manager_application_id",
            "multiarch_tuple",
            "package_name",
            "package_version",
            "product_version",
        ),
    )
    control = subparsers.add_parser("render-control")
    control.add_argument("--output", type=Path)
    shlibdeps_control = subparsers.add_parser("render-shlibdeps-control")
    shlibdeps_control.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        metadata, _ = validate_source_contract()
        if args.command == "field":
            print(getattr(metadata, args.name))
        elif args.command in ("render-control", "render-shlibdeps-control"):
            value = (
                render_control(metadata)
                if args.command == "render-control"
                else render_shlibdeps_control(metadata)
            )
            if args.output is None:
                print(value, end="")
            else:
                if args.output.exists() or args.output.is_symlink():
                    raise LinuxProductMetadataError(
                        "rendered control output must not already exist"
                    )
                args.output.write_text(value, encoding="utf-8")
                args.output.chmod(0o644)
    except (LinuxProductMetadataError, product_data.ProductDataError) as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
