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

The right native binary for your platform is installed automatically via
`optionalDependencies` (gated on `os`/`cpu`), and the bundled `knowledge/`
profiles are wired up for you. Supported: Linux x64, macOS arm64, macOS x64,
Windows x64.

Full documentation: <https://github.com/yudistirosaputro/palugada-cli>
