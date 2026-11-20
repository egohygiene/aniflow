# Managed tool and model commands

Use `aniflow toolchain managed` to inspect or maintain the verified Upscayl and
RIFE cache. The commands use the same public managed store as normal selected
video runs. `doctor` verifies local readiness and reports missing download
costs; it does not launch the native tool or qualify a graphics device.

```sh
aniflow toolchain managed doctor
aniflow toolchain managed plan upscayl-20251207-174704-macos-aarch64-anime4x
aniflow toolchain managed list
```

`doctor` without a package ID shows the bundled selections for the current OS
and architecture. Supply an exact ID to inspect one selection. The bundled
catalog currently includes Linux x86_64 and macOS Intel/ARM rows; run `doctor`
to discover the IDs on your machine. `doctor`, `plan` and `list` do not create
the cache, acquire files or install a cancellation handler.

Acquire the exact selected executable, model pair, license and notices, then
select it for its provider:

```sh
aniflow toolchain managed install upscayl-20251207-174704-macos-aarch64-anime4x
aniflow toolchain managed activate upscayl-20251207-174704-macos-aarch64-anime4x
aniflow toolchain managed install rife-20221029-macos-aarch64-v4.6
aniflow toolchain managed activate rife-20221029-macos-aarch64-v4.6
```

`install` leaves activation as a separate action. `upgrade PACKAGE_ID` installs
and activates the exact catalog selection, retaining older versions.
`rollback PACKAGE_ID` activates an already installed verified selection and
never downloads. No command searches for or silently selects a newer release.

`--offline` and `--no-download` both refuse network acquisition. They allow
installation or repair from already verified cached archives. An unavailable
artifact produces an actionable refusal; it does not trigger a fallback
download. `--timeout` sets each artifact's HTTPS deadline to 1–3600 seconds
(default 300). These flags and `--cache-directory` work before or after the
managed subcommand:

```sh
aniflow toolchain managed --offline plan rife-20221029-linux-x86_64-v4.6
aniflow toolchain managed install rife-20221029-linux-x86_64-v4.6 --no-download
aniflow toolchain managed --cache-directory ./private/managed --timeout 600 install rife-20221029-linux-x86_64-v4.6
aniflow --output json toolchain managed doctor --cache-directory ./private/managed
```

The default root comes from `ANIFLOW_MANAGED_CACHE` when set, otherwise the
platform user cache. An explicitly supplied cache must be a dedicated managed
directory; an existing nonempty unrelated directory is never adopted.
Progress goes to stderr, leaving stdout for the human result or JSON envelope.
The JSON command name is `toolchain_managed`; its result has schema
`aniflow.managed-command/v1` and explicit `operation`, `root` and `data` fields.

Keep a package for a specific run, then release that reference when it is no
longer needed. A reference protects the package from repair or removal:

```sh
aniflow toolchain managed retain rife-20221029-linux-x86_64-v4.6 --run-id demo-run
aniflow toolchain managed release rife-20221029-linux-x86_64-v4.6 --run-id demo-run
aniflow toolchain managed deactivate rife-20221029-linux-x86_64-v4.6
aniflow toolchain managed repair rife-20221029-linux-x86_64-v4.6 --offline
aniflow toolchain managed uninstall rife-20221029-linux-x86_64-v4.6
```

Releasing a reference preserves the run and package files. Deactivation
preserves retained-run references. Repair requires an inactive, unreferenced
package and retains replaced corrupt bytes in quarantine. Uninstall accepts
only an exact verified package ID, and refuses active, retained, unknown or
linked package contents. It does not remove user-supplied binaries/models or
purge shared archive/quarantine evidence.

See [catalog evidence](toolchain-managed-catalog-evidence.md) for exact upstream
sources, model mappings, licenses and OS/runtime bounds. Download readiness,
native processing qualification and media review remain distinct evidence.
