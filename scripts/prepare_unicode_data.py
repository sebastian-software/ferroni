#!/usr/bin/env python3
"""Download and unpack a checksum-pinned Unicode Character Database archive."""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import sys
import tempfile
import tomllib
import urllib.request
import zipfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
PIN_FILE = ROOT / "unicode_data.toml"
CACHE_ROOT = ROOT / ".cache" / "unicode"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "Ferroni Unicode table generator"})
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output)


def safe_extract(archive: Path, destination: Path) -> None:
    with zipfile.ZipFile(archive) as source:
        for member in source.infolist():
            relative = PurePosixPath(member.filename)
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError(f"Unsafe path in UCD archive: {member.filename}")
        source.extractall(destination)


def prepare(version: str) -> Path:
    with PIN_FILE.open("rb") as source:
        pins = tomllib.load(source)["unicode"]
    if version not in pins:
        supported = ", ".join(sorted(pins))
        raise ValueError(f"Unicode {version} is not pinned; available versions: {supported}")
    pin = pins[version]
    expected = pin["sha256"]
    version_dir = CACHE_ROOT / version
    archive = version_dir / "UCD.zip"
    extracted = version_dir / "ucd"
    version_dir.mkdir(parents=True, exist_ok=True)

    if not archive.is_file() or sha256(archive) != expected:
        temporary_archive = version_dir / "UCD.zip.download"
        try:
            print(f"Downloading Unicode {version} UCD from {pin['url']}")
            download(pin["url"], temporary_archive)
            actual = sha256(temporary_archive)
            if actual != expected:
                raise ValueError(f"UCD archive SHA-256 mismatch: expected {expected}, got {actual}")
            os.replace(temporary_archive, archive)
        finally:
            temporary_archive.unlink(missing_ok=True)
    else:
        print(f"Using cached Unicode {version} UCD archive")

    marker = extracted / ".ferroni-source-sha256"
    if marker.is_file() and marker.read_text(encoding="ascii").strip() == expected:
        return extracted

    with tempfile.TemporaryDirectory(prefix=f"ferroni-ucd-{version}-", dir=version_dir) as temporary:
        temporary_root = Path(temporary)
        unpacked = temporary_root / "ucd"
        unpacked.mkdir()
        safe_extract(archive, unpacked)
        if not (unpacked / "UnicodeData.txt").is_file():
            raise ValueError("UCD archive does not contain UnicodeData.txt at its root")
        (unpacked / ".ferroni-source-sha256").write_text(expected + "\n", encoding="ascii")
        if extracted.exists():
            shutil.rmtree(extracted)
        os.replace(unpacked, extracted)
    print(f"Prepared Unicode {version} UCD at {extracted}")
    return extracted


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="Pinned Unicode release, such as 17.0.0")
    args = parser.parse_args()
    print(prepare(args.version))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, KeyError, ValueError, urllib.error.URLError, zipfile.BadZipFile) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
