# palugada-cli

Project-agnostic developer knowledge & connector CLI, distributed as a prebuilt
native binary. No Rust toolchain required.

```bash
npm install -g palugada-cli
palugada --help
palugada q --list          # stack conventions, works offline out of the box

# or without installing
npx palugada-cli q --list
```

On install, the right native binary for your platform is downloaded from the
matching GitHub Release and verified against a checksum bundled in this package,
then extracted next to its `knowledge/` profiles. Supported: Linux x64, macOS
arm64, macOS x64, Windows x64.

If `postinstall` was skipped (`npm ci --ignore-scripts`), the binary is fetched
automatically the first time you run `palugada`. Offline or behind a proxy: set
`PALUGADA_SKIP_DOWNLOAD=1` and grab the archive for your platform manually from
the [Releases page](https://github.com/yudistirosaputro/palugada-cli/releases).

Full documentation: <https://github.com/yudistirosaputro/palugada-cli>
