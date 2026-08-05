# LingStack · 灵栈

LingStack is a local-first desktop hub for discovering, reviewing, verifying, optimizing, and exporting Agent and Skill definitions across AI coding tools.

灵栈是一个本地优先的 Agent / Skill 统一管理器，用于发现、审阅、验真、优化和导出本机 AI 工具中的 Agent 与 Skill 定义。

The public distribution starts with an empty library. It bundles no Agent, Skill, private registry, model credential, or personal configuration. Local discovery starts only after the user triggers it.

## Product preview / 产品预览

![LingStack unified Agent and Skill map](marketing/store-preview/output/zh-CN/01-lingstack.png)

More App Store-ready previews are available in [`marketing/store-preview/output/zh-CN`](marketing/store-preview/output/zh-CN). All eight images use isolated synthetic data and conform to the Mac App Store 2880×1800 screenshot size.

- [Product page / 产品介绍](https://pm.jcm99.com/apple/lingstack/)
- [Privacy policy / 隐私政策](https://pm.jcm99.com/apple/lingstack/privacy.html)
- [Support / 支持](https://pm.jcm99.com/apple/lingstack/support.html)

## Highlights / 主要能力

- 以空资产库启动；手动刷新只读扫描明确支持的工具入口和用户登记的自定义入口。
- 统一浏览 Agent、Skill、项目能力图谱和版本处置队列。
- 按名称、能力、工具、项目、许可证、状态和版本风险筛选。
- 自动发现 OpenAI-compatible、Ollama 等端点的多个模型，并允许用户选择默认模型。
- 直接验真 Agent/Skill 的结构、可读性和元数据，生成优化建议与治理回执。
- 工具调用路径变更始终需要用户预览并手动触发。
- 导出 JSON、Markdown 或脱敏 ZIP；导出过滤密钥、令牌、Cookie、数据库、依赖、缓存、符号链接和超大文件。
- 后台窗口暂停轮询，大列表分页，耗时扫描在阻塞线程中执行。

## Supported languages / 支持语言

简体中文、繁體中文、English、日本語、한국어、Français、Deutsch、Español、Português、Русский、العربية。

## Core principles / 核心原则

1. 本机发现不等于已安装、已加载、已验证或可发布。
2. 多版本冲突必须由用户选择，不自动覆盖固定版本。
3. 外部候选默认不安装、不执行。
4. 可能修改路径或文件的操作必须由用户手动触发。
5. 发行包和源码仓库均不包含任何用户自己的 Agent 或 Skill。

## Technology / 技术栈

- Tauri 2
- Svelte 5 + TypeScript
- Rust
- macOS Keychain

## Development / 开发

Requirements: Node.js 22+, Rust stable, and Xcode Command Line Tools.

```bash
npm ci
npm run check:i18n
npm run check
npm run build
npm run test:rust
npm run tauri dev
```

Build a Developer ID-signed clean macOS release:

```bash
npm run release:clean
```

## Repository layout / 目录

```text
src/                         Svelte application and design system
src/lib/i18n.ts              11-language catalog and locale runtime
src/lib/components/          Feature panels
src-tauri/src/registry.rs    Local discovery and index
src-tauri/src/control.rs     Models and explicit tool-path control
src-tauri/src/verification.rs Agent/Skill verification
src-tauri/src/governance.rs  Optimization and governance receipts
src-tauri/src/export.rs      Safe exports
scripts/check-i18n.mjs       Translation completeness gate
scripts/audit-clean-release.py Public artifact privacy audit
app-store/                   11-locale App Store metadata
marketing/                   Synthetic demo fixture and promotional screenshots
site/apple/lingstack/        Public product, privacy, and support pages
```

## Privacy boundary / 隐私边界

- The public app starts with zero assets and does not read or upload a private registry.
- Discovery runs only after the user triggers it.
- Agent and Skill definitions stay local unless the user explicitly exports them.
- Model credentials are stored in macOS Keychain; requests go only to endpoints configured by the user.
- The app bundles no analytics or advertising SDK.

See [Privacy](docs/PRIVACY.md), [Architecture](docs/ARCHITECTURE.md), [Security Policy](SECURITY.md), and [Contributing](CONTRIBUTING.md).

## License

Apache License 2.0. Copyright © 2026 野路子工作室.
