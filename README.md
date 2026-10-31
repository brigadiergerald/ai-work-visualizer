# AI Work Visualizer (Development Version)

English | [日本語](README.ja.md)

A local desktop app for Windows that lets you follow Codex workflows, review decisions, and inspect deliverables. This repository provides **development source code for Windows and Codex users**, rather than an installer or a finished release.

All characters in the current team stay visible at the top. The left pane contains requests, decisions, and activity; the right pane contains document content, deliverables, and posting preparation. The current social media workflow uses four characters, while historical five-stage workflows use five. Artwork for all five characters is included.

## Development setup and launch

- Windows x64 (the current build and offline tests were run on Windows 10)
- Node.js 24 and npm
- Rust stable (MSVC), Visual Studio C++ Build Tools, the Windows SDK, and WebView2
- Your own Codex installation, signed in with your ChatGPT account
- Python 3 for backend offline tests

```powershell
npm ci
npm run desktop
```

To build an executable from source:

```powershell
npm run desktop -- -Build
```

The output is `src-tauri/target/release/ai-guild.exe`. This command does not sign the app or create an installer. See the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for the Rust and Windows requirements.

## Connecting and running

In Settings, check the working directory, output directory, and Codex executable, then run the connection check. The app will not start work if it cannot verify Codex's effective permissions and the Windows sandbox. Credentials and Codex itself are not included.

Publishing or using this source does not provide a Codex usage allowance. Your own account's terms and usage limits apply. This is not an official OpenAI product. See the [official Codex App Server documentation](https://learn.chatgpt.com/docs/app-server) for authentication requirements.

For the app-server authentication used here, the official documentation distinguishes continued use in local or open-source applications from use in commercial or hosted services. This authentication method must not be reused as-is for a commercial or hosted service. The Apache-2.0 license for this project's code is separate from external services' authentication requirements and terms of use.

## Current scope

- Social media writing workflows, approvals, deliverable review, copying text, and opening saved files
- Keeping all team characters visible, recording decisions and activity, and displaying historical workflows
- Preparing posts after verifying the connected account and its status

Posting provider implementations are included, but users must configure the connection, verify the destination, and explicitly submit each post. Launching the app or seeing a character marked complete does not publish a post. Unsupported social networks cannot be used for submission. Connection checks and posting involve external communication, so start with an isolated working directory and test input.

This is under development and may lag behind changes to Codex or external services. Back up existing data before running it in an environment that already uses the same application ID. This initial release has a limited scope; end-to-end operation on a new PC and publishing to live services are not universally verified.

## Tests

```powershell
npm run check
npm run build
npm run test:boundary
npm run test:native
```

The standard tests use offline mocks and public sample fixtures. They do not run Codex inference or publish real posts. Fixture IDs and paths have been replaced with examples and are not execution evidence. Malformed ZIP fixtures are inputs for rejection tests; do not manually extract or install them.

## Licensing

**Project code and documentation are licensed under Apache-2.0. Character artwork has separate terms.** The repository as a whole must not be treated as an Apache-2.0 asset pack.

- [LICENSE](LICENSE): Apache License 2.0 for project code and documentation
- [LICENSING.md](LICENSING.md): license scope
- [CHARACTER_ASSETS_LICENSE.md](CHARACTER_ASSETS_LICENSE.md): bundling and modifying the characters is allowed; standalone redistribution of the characters is prohibited
- [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md): third-party dependencies

Being able to view or fork the repository on GitHub does not grant permission to redistribute the artwork separately. Keep the artwork's license terms with any permitted redistribution.
