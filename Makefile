.PHONY: clean clean_all

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXTENSION_NAME=healpix

# Set to 1 to enable Unstable API (binaries will only work on TARGET_DUCKDB_VERSION, forwards compatibility will be broken)
# Note: currently extension-template-rs requires this, as duckdb-rs relies on unstable C API functionality
# See docs/adr/0003-duckdb-version-pin.md
USE_UNSTABLE_C_API=1

# Target DuckDB version
TARGET_DUCKDB_VERSION=v1.5.6

all: configure debug

# Include makefiles from DuckDB
include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

# AIDEV-INVARIANT: `test_debug` and `test_release` must stay exactly as the
# upstream template defines them. The community-extensions pipeline invokes
# `make test_$(build_type)` directly -- for linux, inside a docker container
# (extension-ci-tools/.github/workflows/_extension_distribution.yml:464) -- on a
# tree it has already built. Adding build or cargo prerequisites to these
# targets would re-run the build inside that container and make our extension
# depend on a Rust toolchain being present in the test image.
test_debug: test_extension_debug
test_release: test_extension_release

# `make test` is the LOCAL entry point and is what AGENTS.md requires before
# declaring a step done. README defines it as cargo test (unit + proptest) plus
# sqllogictest, and `debug` is a prerequisite so a clean checkout builds the
# extension before the sqllogictest runner tries to LOAD it. CI never calls it.
test: test_rust debug test_extension_debug

.PHONY: test_rust
test_rust:
	cargo test

clean: clean_build clean_rust
clean_all: clean_configure clean
