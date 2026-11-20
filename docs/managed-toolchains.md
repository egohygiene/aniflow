# Managed external tools and models

Aniflow's public `toolchain::managed` library prepares verified, versioned
Upscayl and RIFE executable/model packages for the existing media owners.
Selected normal execution can acquire its missing curated dependencies. It
does not download every provider, select interpolation, install drivers, change
explicit user paths, or process media itself. Read-only plans never acquire or
write anything. `offline` acquisition uses exact verified installed bytes or
verified cached archives and refuses network when those are missing.

The [compiled catalog](../providers/managed/catalog.v1.json) and
[catalog schema](contracts/managed-catalog-v1.schema.json) are shipped with the
source package. Catalog updates require a source change; no remote response
changes trusted policy. Its nine selections cover anime/general Upscayl and
RIFE v4.6 for Linux x86_64 and both Mac architectures. Package ids are immutable
and bind the provider release/source, host, backend, model, URLs, archive and
member hashes, lengths, installed layout and retained license notices. See the
[actual asset and runtime evidence](toolchain-managed-catalog-evidence.md) for
upstream origins, license scope, Mac OS floors, Linux system dependencies and
qualification limits. Existing explicit inventories/configurations remain
usable through their existing owners.

## Library flow

`ManagedStore::new(root)` validates a dedicated user-owned store path without
creating it. `bundled_catalog()` returns the closed, validated catalog;
`catalog.package(id)` selects an exact entry. `store.plan(&catalog, id, offline)`
is a read-only inspection of full installed identities and exact cached archive
availability. It returns `ready`, `cached`, `missing` or `corrupt`, download bytes,
selected installed bytes and missing artifact ids. An empty precreated dedicated
root is eligible for first initialization; a populated unowned directory is
refused.

`store.acquire(&catalog, id, &HttpsDownloader, &control)` is the separate effectful
boundary. It obtains only the selected package, returns an
`InstalledManagedPackage`, and exposes its verified executable, model and runtime
paths. Acquisition does not activate or run a provider. The caller resolves
selected manual overrides first, then feeds these exact paths and receipt
identities to the existing setup/recovery/interpolation owners. No per-frame
downloader or alternate media runner exists. Before consuming managed paths for
a retained run, the caller must durably call `retain(package_id, run_id)` under
the same store lock used for lifecycle operations. A stable source/configuration
digest is a suitable opaque run id; personal paths do not belong in it.

The receipt schema is `aniflow.managed-receipt/v1`. Each receipt retains the whole
exact package declaration, its serialized SHA-256, original catalog revision and
installation time. `receipt.identity()` projects the package id, package SHA-256
and catalog revision for downstream immutable configurations. A new catalog
revision may reuse an unchanged package; it cannot repoint an installed id to a
different declaration. Receipt provenance is neither a publisher signature nor
a native execution observation.

## Acquisition and ownership

The store contains `owner`, `writer.lock`, `versions/<package-id>/receipt.json`,
selected version files, content-addressed `archives/`, private `staging/`,
`quarantine/`, and optional `references.json`. The writer holds an exclusive
nonblocking OS lock on Unix, released on process exit. Another mutation receives
a clear busy error. The non-Unix fallback uses an exclusive create lock requiring
explicit reconciliation after a crash; this catalog has no Windows selection.

The default transport invokes curl directly, disables its configuration file,
uses no shell or credentials, enforces HTTPS, a shared request timeout, exact
maximum body length, a 64 KiB header limit and at most four individually checked
redirects. Only GitHub's declared public release/raw/asset hosts are allowed.
Cancellation kills and reaps curl; stream writes are bounded. Public callers can
provide a different `ManagedDownloader`; the store still verifies actual length
and SHA-256 and never treats transport success as integrity evidence.

Before extraction, every ZIP member is checked, including unselected members.
Absolute/traversing/ambiguous paths, portable reserved names, case collisions,
symlinks, devices and other special files are refused. The installer extracts
only catalog-selected regular files through create-new writes and checks every
member's actual digest and length. Limits are 1 GiB per acquired artifact, 2 GiB
aggregate uncompressed ZIP bytes and installed bytes, and 30,000 archive/selected
members. This permits the existing approximately 436 MB RIFE distribution while
installing only the selected v4.6 model pair. It preserves relative installed
runtime paths and executable permissions.

All bytes, closed member inventory and receipt are checked in private staging,
then files/directories are synced and the complete directory is atomically
renamed into its immutable version slot. No partial download or failed extraction
is reported ready. A later inspection rehashes every selected file and rejects
extra files/directories, symlinks, hardlinks or a changed specification. A crash
may leave a staging directory for inspection; it does not become an installation.
Verified archives can be reused on retry. Failed/corrupt retained material is
preserved in quarantine rather than recursively erased as guessed temporary data.

## Selected lifecycle

`list` and `inspect` report verified installations. `repair` acquires or rebuilds
the exact selected package and quarantines corrupt prior bytes; it refuses to
replace active or retained-run versions and refuses to repoint an immutable id.
`upgrade` installs an explicitly chosen catalog package and activates it, keeping
the old version. `activate` and `rollback` select already installed verified
versions; rollback performs no network acquisition. `deactivate` removes only an
active selection. `references`, `retain` and `release_retention` inspect or edit
specific durable version/run references. Releasing a reference does not delete
the run itself, and should happen only when its resume requirements are retired.

`uninstall(package_id)` removes only the verified, unreferenced managed version's
declared files and exact ancestor directories. It refuses active/retained versions,
unknown directory contents, linked files and user paths. It retains verified
shared download archives; it does not uninstall GUI applications, manually
supplied models, system libraries, GPU drivers or any external directory. A
corrupt referenced version must first be reconciled with the run that references
it. Store deletion is never a substitute for run retention decisions.

## Validation scope

The focused integration tests exercise injected corrupt/interrupted downloads,
read-only and offline behavior, ZIP allowlists/traversal/links/case collisions,
atomic readiness, exact cache reuse/corruption, concurrent writers, quarantine,
version activation/rollback, retained references, and refusal of user/unknown/
hardlinked files. Their fixture bytes are not native tool or model evidence.
An explicitly ignored public-download test exercises the default HTTPS adapter
and actual selected official Linux Upscayl archive/model installation followed
by exact offline readback. It executes no provider. Native Mac/GPU processing,
perceptual quality and complete end-to-end release qualification remain separate
observations; availability and verified installation do not make those rows green.
