# Architecture

LingStack uses a Svelte 5 frontend inside a Tauri 2 desktop shell. Rust commands provide local discovery, hashing, verification, governance receipts, model endpoint access, and safe exports.

The public repository enables the `public-release` Cargo feature by default. It creates an empty application-owned index and never bundles Agent/Skill definitions. Discovery follows an allowlisted set of tool locations plus user-defined bindings, does not follow symbolic links, and is only triggered by the user.

Security boundaries:

- Tauri capabilities expose only the required dialog and opener permissions.
- The webview uses a restrictive content security policy.
- Model secrets are stored in macOS Keychain rather than JSON configuration.
- Export filters reject secret-shaped files, credentials, caches, dependencies, symlinks, and oversized content.
- Tool path changes require an explicit preview and user-triggered apply operation.
