#!/usr/bin/env python3

from pathlib import Path
import json
import os
import shutil
import subprocess
import sys
import tempfile

SCRIPT_DIR = Path(__file__).parent

if __name__ == "__main__":

    metadata_dir = SCRIPT_DIR / "metadata"
    cargo = os.getenv("CARGO", "cargo")
    
    with tempfile.TemporaryDirectory() as temp_dir:
        temp_dir_path = Path(temp_dir)
        temp_dir_path.mkdir(parents=True, exist_ok=True)

        for test_dir in metadata_dir.iterdir():

            # Check to see if the directory contains a Cargo manifest
            real_manifest = test_dir / "Cargo.toml"
            if not real_manifest.exists():
                continue

            # Copy the test directory into a temp directory (and out from under a Cargo workspace)
            manifest_dir = temp_dir_path / test_dir.name
            # manifest_dir.mkdir(parents=True, exist_ok=True)
            shutil.copytree(test_dir, manifest_dir)

            manifest = manifest_dir / "Cargo.toml"
            
            # Generate metadata
            proc = subprocess.run(
                [cargo, "metadata", "--format-version", "1", "--manifest-path", str(manifest)],
                capture_output=True)

            if proc.returncode:
                print("Subcommand exited with error", proc.returncode, file=sys.stderr)
                print("Args:", proc.args, file=sys.stderr)
                print("stderr:", proc.stderr.decode("utf-8"), file=sys.stderr)
                print("stdout:", proc.stdout.decode("utf-8"), file=sys.stderr)
                exit(proc.returncode)

            # Write metadata to disk
            metadata = json.loads(proc.stdout)
            output = test_dir / "metadata.json"
            output.write_text(json.dumps(metadata, indent=4))
