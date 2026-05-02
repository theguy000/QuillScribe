# Building QuillScribe on Windows

## Prerequisites

### 1. Rust Toolchain

Install Rust via [rustup](https://rustup.rs/):

```powershell
winget install Rustlang.Rustup
```

Ensure the `stable-x86_64-pc-windows-msvc` target is installed (default on Windows).

### 2. Visual Studio Build Tools

Install Visual Studio Build Tools with the C++ workload. This provides the MSVC compiler required by whisper.cpp:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

During installation, select **"Desktop development with C++"**.

### 3. LLVM / Clang

Required by `bindgen` to generate Rust bindings for whisper.cpp:

```powershell
winget install LLVM.LLVM
```

Then set the environment variable permanently:

```powershell
[Environment]::SetEnvironmentVariable('LIBCLANG_PATH', 'C:\Program Files\LLVM\bin', 'User')
```

### 4. CMake

CMake is required to compile whisper.cpp from source. If you have Visual Studio Build Tools installed, CMake is bundled at:

```
C:\Program Files (x86)\Microsoft Visual Studio\<version>\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe
```

Set the `CMAKE` environment variable so the Rust build script can find it:

```powershell
# Adjust the path to match your Visual Studio version
[Environment]::SetEnvironmentVariable('CMAKE', 'C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe', 'User')
```

Alternatively, install CMake standalone and add it to PATH:

```powershell
winget install Kitware.CMake
```

### 5. Slint system dependencies

No additional dependencies needed on Windows — Slint uses the system's Direct3D or Vulkan via wgpu.

## Environment Variables Summary

After installing prerequisites, ensure these user environment variables are set. Open a **new** PowerShell window after setting them:

| Variable | Value | Purpose |
|---|---|---|
| `LIBCLANG_PATH` | `C:\Program Files\LLVM\bin` | `bindgen` needs `libclang.dll` to generate FFI bindings |
| `CMAKE` | `<path to cmake.exe>` | whisper.cpp build script needs CMake |

You can verify they are set correctly:

```powershell
[Environment]::GetEnvironmentVariable('LIBCLANG_PATH', 'User')
[Environment]::GetEnvironmentVariable('CMAKE', 'User')
```

**Important:** Environment variable changes via `SetEnvironmentVariable` or `setx` only take effect in **new** terminal sessions. Close and reopen your terminal after setting them.

## Building

### Development

```bash
cargo run
```

The first build will take several minutes as it compiles whisper.cpp from source.

### Production

```bash
cargo build --release
```

This produces the binary at `target/release/quillscribe.exe`.

## Troubleshooting

### `is 'cmake' not installed?`

The `CMAKE` environment variable is not set, or you haven't opened a new terminal since setting it. Verify:

```powershell
$env:CMAKE
```

If empty, set it as described above and **restart your terminal**.

### `Unable to find libclang`

The `LIBCLANG_PATH` environment variable is not set. Verify:

```powershell
$env:LIBCLANG_PATH
```

If empty, set it as described above and **restart your terminal**.

### `attempt to compute ... which would overflow` (struct size mismatch)

This occurs when `whisper-rs` and `whisper-rs-sys` versions are incompatible. Ensure `Cargo.toml` uses `whisper-rs = "0.16"` or later. Run:

```bash
cargo update -p whisper-rs
cargo update -p whisper-rs-sys
```

### First build is very slow

Normal. The first build compiles whisper.cpp from source (~5-10 minutes). Subsequent builds are incremental and much faster.

### Local whisper model not working

Ensure you have downloaded a model first via the Settings dialog (Whisper tab > Local Model > Download). Models are cached at:

```
%APPDATA%\..\Local\quillscribe\models\
```

Or more precisely, whatever `dirs::config_dir()` resolves to on your system, under `quillscribe/models/`.
