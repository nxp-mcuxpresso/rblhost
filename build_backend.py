# Copyright 2025 NXP
#
# SPDX-License-Identifier: BSD-3-Clause
"""Custom build backend that generates stub files before building."""

from setuptools.build_meta import *
from setuptools.build_meta import build_wheel as _build_wheel
from setuptools.build_meta import build_sdist as _build_sdist
from setuptools.build_meta import build_editable as _build_editable
import subprocess
import shutil
import os
import sys


def _generate_stubs():
    """Generate stub files before building."""
    # Check if we're in an sdist (no Cargo.toml means we're in sdist)
    if not os.path.exists("Cargo.toml"):
        print("Skipping stub generation in sdist build")
        return

    print("Generating Python stub files...")

    try:
        # Run the stub generator
        result = subprocess.run(
            ["cargo", "run", "--bin", "stub_gen", "--features", "python"],
            check=True,
            capture_output=True,
            text=True,
        )
        print(f"Stub generation output: {result.stdout}")

        # Ensure the pymboot directory exists
        os.makedirs("pymboot", exist_ok=True)

        # Copy the generated stub file to the package directory
        if os.path.exists("pymboot.pyi"):
            shutil.copy("pymboot.pyi", "pymboot/__init__.pyi")
            print("Copied pymboot.pyi to pymboot/__init__.pyi")
        else:
            print("Warning: pymboot.pyi was not generated!")

        # Create py.typed marker for PEP 561 compliance
        with open("pymboot/py.typed", "w") as f:
            pass
        print("Created py.typed marker")

    except subprocess.CalledProcessError as e:
        print(f"Error generating stubs: {e}")
        print(f"stdout: {e.stdout}")
        print(f"stderr: {e.stderr}")
        raise


def build_wheel(wheel_directory, config_settings=None, metadata_directory=None):
    """Build wheel with stub generation."""
    _generate_stubs()
    return _build_wheel(wheel_directory, config_settings, metadata_directory)


def build_sdist(sdist_directory, config_settings=None):
    """Build source distribution with stub generation."""
    _generate_stubs()
    return _build_sdist(sdist_directory, config_settings)


def build_editable(wheel_directory, config_settings=None, metadata_directory=None):
    """Build editable install with stub generation."""
    _generate_stubs()
    return _build_editable(wheel_directory, config_settings, metadata_directory)
