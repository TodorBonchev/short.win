#!/usr/bin/env python3
"""Package only allow-listed promo assets and sources; never home-directory files."""

import hashlib
import json
from pathlib import Path
import zipfile

project = Path(__file__).resolve().parents[1]
repo = project.parent.parent
name = "ShortWin-promo-editable-v1.0.3"
output = project / "out" / f"{name}.zip"
output.parent.mkdir(exist_ok=True)

files = [
    project / entry
    for entry in (
        "README.md", "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml",
        "tsconfig.json", ".gitignore", "out/export-manifest.json",
    )
]
for directory in ("src", "scripts", "public"):
    files.extend(path for path in (project / directory).rglob("*") if path.is_file())
# Successful speech responses contain audio/alignment, not authentication headers.
# They are sufficient to regenerate voice, captions and music with --offline.
files.extend((project / ".cache/audio").glob("*.response.json"))
files.extend(repo / entry for entry in (
    "LICENSE", "REUSE.toml", "THIRD_PARTY_NOTICES.md", "TODO.md",
    "LICENSES/LicenseRef-Proprietary.txt", "LICENSES/CC-BY-NC-ND-4.0.txt", "LICENSES/OFL-1.1.txt",
    "docs/demo/local-validator-v1.0.3-2026-10-20.md",
    "docs/demo/local-validator-phone-v1.0.3-2026-10-20.md",
    "docs/demo/narration-v1.0.3-2026-10-20.md",
    "docs/demo/promo-v1.0.3-2026-10-21.md",
    "docs/demo/promo-v1.0.3-export.json",
    "docs/design/design-pass-2026-10/artifacts/logo/final/mark.svg",
    "docs/design/design-pass-2026-10/artifacts/logo/final/lockup-two-tone.svg",
    "apps/web/e2e/demo/README.md",
))
# Keep the original silent sources too, so prepare:footage works after extraction.
for directory in (repo / "apps/web/.e2e/demo", repo / "apps/web/.e2e/demo/phone"):
    manifest = directory / "render.json"
    source = directory / Path(json.loads(manifest.read_text())["mp4"]).name
    files.extend((manifest, source))

files = sorted(set(files))
for path in files:
    if not path.is_file() or path.is_symlink():
        raise RuntimeError(f"Missing or unsupported bundle input: {path}")
    path.resolve().relative_to(repo)

guide = """ShortWin promotional video project — v1.0.3

Open a terminal in this extracted folder:
  cd tools/promo
  pnpm install --frozen-lockfile
  pnpm studio

Both compositions and all prepared media are ready; no API call is required.
Use pnpm render to export. Node 24.21+, pnpm 12.7, FFmpeg and Chromium are needed.
If Chromium is elsewhere, update its path in scripts/render.mjs.
The included original captures and response cache also allow source preparation
and offline soundtrack regeneration. No credentials or installed dependencies
are included. See tools/promo/README.md for provenance and editing instructions.
"""
with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
    archive.writestr(f"{name}/START-HERE.txt", guide)
    for path in files:
        archive.write(path, f"{name}/{path.relative_to(repo)}")
with zipfile.ZipFile(output) as archive:
    bad = archive.testzip()
    if bad:
        raise RuntimeError(f"Bundle CRC check failed: {bad}")
print(json.dumps({
    "file": str(output), "files": len(files) + 1,
    "bytes": output.stat().st_size,
    "sha256": hashlib.file_digest(output.open("rb"), "sha256").hexdigest(),
}, indent=2))
