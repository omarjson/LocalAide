# Contributing to LocalAide

Thank you for your interest in contributing. LocalAide is a local-first AI desktop assistant built with Rust + Tauri v2.

## How to Contribute

1. Fork the repository and create a feature branch.
2. Ensure `cargo check` and `npm run build` pass.
3. Follow existing code style. Rust code uses `cargo fmt` and `clippy`.
4. Add tests where applicable.
5. Submit a Pull Request with a clear description.

## Development Setup

Requirements:
* Rust 1.90+ (stable toolchain)
* Node 20+
* Tauri prerequisites for your platform

```bash
git clone https://github.com/<user>/localaide.git
cd localaide
npm ci
npx tauri dev
```

## Project Structure

* `src-tauri/` - Rust backend
* `src/` - Svelte 5 frontend
* `docs/` - Documentation, ADRs, and evidence

## Reporting Issues

Please use GitHub Issues. Include OS, version, steps to reproduce, and logs from `tauri-plugin-log`.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
