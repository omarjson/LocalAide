# Third-party notices

LocalAide is MIT licensed (see `LICENSE`). It includes open-source software made by others. This file lists the parts that ship inside the app and their licenses.

## Bundled programs

| Part | What it does | License |
|---|---|---|
| [llama.cpp](https://github.com/ggml-org/llama.cpp) and ggml, as `llama-server` | Runs the local models. Not bundled: downloaded from the official llama.cpp release on first use | MIT, Copyright (c) 2023-2024 The ggml authors |
| [SQLite](https://sqlite.org) (compiled in through `rusqlite`) | Stores the library, notes and activity log | Public domain |
| [Tauri](https://tauri.app) | The app window and its link to the system | Apache-2.0 or MIT |
| Microsoft Edge WebView2 bootstrapper (Windows installer only) | Installs the system web view if it is missing | Microsoft WebView2 distribution terms |

## The app's code libraries

- **Rust packages.** Release builds include `THIRD_PARTY_LICENSES.html`, made by `cargo about` from `src-tauri/Cargo.lock`. It holds the full license text of every Rust package in the app.
- **Web packages.** The app window bundles [Svelte](https://svelte.dev) (MIT), [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) (Apache-2.0 or MIT) and styles built with [Tailwind CSS](https://tailwindcss.com) (MIT).
- **Every package.** Each GitHub release has an SPDX software bill of materials (`localaide-sbom.spdx.json`) that names every Rust and npm package, its version and its license.

## Models

LocalAide does not ship any model. Models you download from the built-in list or from Hugging Face keep their own licenses. The Models screen shows each license before you download.

## MIT License text (llama.cpp and ggml)

```
MIT License

Copyright (c) 2023-2024 The ggml authors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
