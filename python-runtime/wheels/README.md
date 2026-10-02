# Windows CPython 3.13 compression dependency

`lzfse-0.4.2-cp313-cp313-win_amd64.whl` was built locally from the unchanged upstream `lzfse` 0.4.2 source. It is a third-party dependency, not original Meridian source. The published release has Windows wheels through CPython 3.12; this wheel lets the pinned dependency graph run on CPython 3.13, whose native TLS-PSK API is used by pymobiledevice3.

| Item | Provenance |
|---|---|
| Source | [lzfse 0.4.2 on PyPI](https://pypi.org/project/lzfse/0.4.2/), `lzfse-0.4.2.tar.gz`, uploaded 19 April 2024 |
| Source archive SHA-256 | `c6895f8ca13eedd2e18b6e0c987c9468115030898409bc44a7aa8d4f4a42ceab` |
| Supplied wheel SHA-256 | `4e8965eedf7e4b7526fb50cadec939b53bad668272a31ee7572b9d0e7b05a97b` |
| Interpreter | CPython 3.13.15, Windows x64, from [Astral python-build-standalone](https://github.com/astral-sh/python-build-standalone) |
| Build tools | setuptools 84.0.0; wheel 0.47.0; LLVM-MinGW 20260826 UCRT x86_64, using setuptools' `mingw32` compiler mode |
| Source changes | None; upstream `setup.py`, Python binding, and LZFSE C/header sources retained |
| Local audit evidence | Workspace `work/lzfse-candidate-source/` and `work/lzfse-candidate-build.log`; these scratch files are outside the source archive |

The wheel is pinned by relative path and SHA-256 in `python-runtime/requirements.lock`. Install that lock from the repository root with CPython 3.13 x64. A different interpreter ABI or platform requires a separately reviewed wheel/lock.

To rebuild, download the source archive from the linked PyPI release, verify its SHA-256, and extract it into a new build directory. In a CPython 3.13.15 x64 environment with the listed setuptools and wheel versions, put the listed LLVM-MinGW `bin` directory at the front of that build process's PATH. From the extracted source directory, run:

```powershell
python setup.py build_ext --compiler=mingw32 bdist_wheel --dist-dir "<wheel output directory>"
```

This is the upstream build entry point used for the supplied wheel. No global compiler or security setting was changed. Build logs include upstream setuptools deprecation warnings and harmless Clang warnings about an unused `-shared` compilation argument. A rebuild may differ byte-for-byte because timestamps and build paths were not normalized; review a replacement wheel and update the lock hash deliberately.

The wheel retains `lzfse-0.4.2.dist-info/licenses/COPYING`, `LICENSE.lzfse`, and `AUTHORS`. `COPYING` covers the MIT-licensed Python wrapper; `LICENSE.lzfse` contains Apple's separate redistribution terms for the bundled reference implementation. Preserve all three. The upstream project's metadata alone does not replace the included native-code notice.

The local wheel imports successfully and a synthetic compression/decompression round trip passes. The source runtime tests and frozen offline checks provide separate integration evidence; they do not establish physical Wi-Fi reachability or a complete distribution license audit.
