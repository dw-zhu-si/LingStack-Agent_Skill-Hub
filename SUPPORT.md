# LingStack Support / 灵栈支持

LingStack is a local-first macOS application for managing Agent and Skill definitions. This page is the official support contact for the GitHub and Mac App Store editions.

灵栈是一款本地优先的 macOS Agent 与 Skill 管理器。本页是 GitHub 版与 Mac App Store 版的官方支持入口。

## Contact / 联系方式

- Email / 邮箱：[n_sir@sina.com](mailto:n_sir@sina.com)
- Public bug reports / 公开问题：[GitHub Issues](https://github.com/dw-zhu-si/LingStack-Agent_Skill-Hub/issues)
- Security or privacy reports / 安全与隐私问题：请通过邮件私下报告，不要在公开 Issue 中附加凭证、私人 Agent/Skill 内容或完整日志。

When requesting support, include the LingStack version, macOS version, the action you were performing, the exact error message, and minimal reproduction steps. Do not send API keys, access tokens, private configuration, personal paths, or confidential Agent/Skill content.

请求支持时，请提供灵栈版本、macOS 版本、正在执行的操作、完整错误提示和最小复现步骤。请勿发送 API 密钥、访问令牌、私人配置、个人路径或机密 Agent/Skill 内容。

## Common questions / 常见问题

### The app starts with no Agents or Skills / 首次打开没有 Agent 或 Skill

Public releases intentionally contain no personal assets. Add a folder with the macOS folder picker, then start a local refresh. LingStack only indexes locations you explicitly authorize.

公开发行版不会内置任何个人资产。请先通过 macOS 文件夹选择器添加目录，再手动刷新；灵栈只索引你明确授权的位置。

### A model list cannot be retrieved / 无法获取模型列表

The Mac App Store edition supports only Ollama running on this Mac at localhost or a loopback address, without model credentials. Confirm that Ollama is running and has a local model installed. The GitHub edition also supports configured model endpoints; check their URL and authorization. Connectivity checks and minimal inference checks are separate actions. Never post credentials in an Issue.

Mac App Store 版仅支持本机 localhost 或回环地址上的 Ollama，不使用模型凭证。请确认 Ollama 已运行并安装本地模型。GitHub 版也支持自行配置模型接口，请检查 URL 和授权。连接检查与最小推理验真是两个独立操作；请勿在 Issue 中公开凭证。

### Which asset version should I choose? / 多个资产版本如何选择

Open the asset details and read both versions in the comparison panel. The selection helper explains its suggestion using the registered source, current selection, and valid verification records. A matching source is not proof of compatibility, and a longer or newer definition is not necessarily better. When evidence is insufficient, the helper does not choose a version. Its button only fills the candidate field; review and confirm the choice separately.

打开资产详情，在对比面板读取两个版本。“帮我选择”会结合登记来源、当前选择和有效验证记录解释建议。来源匹配不等于兼容性已验证，内容更长或更新也不代表更好。证据不足时不会指定版本。建议按钮只预填候选，仍需你检查并单独确认。

### Tool paths are not changed automatically / 工具路径不会自动切换

This is intentional. The GitHub edition previews path changes and requires explicit confirmation. The sandboxed Mac App Store edition does not rewrite other tools' configuration paths.

这是安全设计。GitHub 版会先预览路径变更并要求明确确认；沙盒化的 Mac App Store 版不会改写其他工具的配置路径。

### Remove local data / 删除本机数据

Remove saved model profiles in LingStack first, then delete the app's local support data and any corresponding macOS Keychain entries. Email support if you need step-by-step assistance for your installed edition.

请先在灵栈中删除已保存的模型配置，再移除应用的本地支持数据和对应的 macOS 钥匙串项目。如需针对已安装版本的逐步指导，请通过邮件联系。

## System requirements / 系统要求

- macOS 10.15 or later / macOS 10.15 或更高版本
- Apple silicon or Intel Mac / Apple 芯片或 Intel Mac

The project is maintained on a best-effort basis. Support requests are reviewed through the email address and public issue tracker above.

本项目通过上述邮箱与公开问题跟踪器处理支持请求，并将尽力响应。
