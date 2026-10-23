"""Download the tree-sitter grammars (.wasm) of Carbon's official extensions.

Each entry in `grammars.toml` names a file inside an extension folder, the
upstream release asset it comes from, and its SHA-256. The script downloads
missing or changed files and refuses any whose hash does not match, so the
grammars in the repo are exactly the pinned upstream builds.

Usage: python extensions/fetch_grammars.py [--check]
  --check  only verify the files already present (exit 1 on a mismatch)
"""

import hashlib
import sys
import tomllib
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    check_only = "--check" in sys.argv[1:]
    entries = tomllib.loads((ROOT / "grammars.toml").read_text("utf-8"))["grammar"]
    failed = 0
    for entry in entries:
        target = ROOT / entry["file"]
        if target.is_file() and sha256(target.read_bytes()) == entry["sha256"]:
            print(f"ok       {entry['file']}")
            continue
        if check_only:
            print(f"MISMATCH {entry['file']}")
            failed += 1
            continue
        with urllib.request.urlopen(entry["url"], timeout=60) as response:
            data = response.read()
        digest = sha256(data)
        if digest != entry["sha256"]:
            print(f"REFUSED  {entry['file']}: got sha256 {digest}")
            failed += 1
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        print(f"fetched  {entry['file']} ({len(data) // 1024} KB)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
