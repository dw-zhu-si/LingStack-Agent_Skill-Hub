Build 111 addresses both issues from Submission c4e0d610-eb7a-4bf4-960b-42fa156a7121.

Mainland China compliance:

- The Mac App Store edition no longer includes OpenAI- or ChatGPT-related access, configuration, credentials, endpoints, labels, or fallback behavior.
- Model access in this edition is limited to a locally running Ollama service on localhost or a loopback IP address. Remote hosts and all model credentials are rejected by the backend, not merely hidden in the interface.
- When upgrading from build 110, any previously saved non-local model profile is removed and its corresponding Keychain credential is deleted.
- All 11 storefront localizations were revised to remove the identified references. The marketing URL was cleared, and the previous model-access promotional screenshot was removed from the App Store screenshot set.
- The separately distributed GitHub edition is not the binary submitted to the Mac App Store.

Support:

- The Support URL now points to a public bilingual support page with an email address, public issue tracker, system requirements, troubleshooting guidance, and privacy instructions:
  https://github.com/dw-zhu-si/LingStack-Agent_Skill-Hub/blob/main/SUPPORT.md

Suggested review steps:

1. Launch LingStack and open Unified Control > Model Access.
2. Edit or add a profile. Only Ollama is available in the Mac App Store build.
3. Entering a non-loopback host is rejected. The credential fields are not present.
4. Agent and Skill inventory, project mapping, verification, version governance, optimization drafts, and clean export remain available using folders selected explicitly through the macOS system picker.

No personal Agent, Skill, credential, or machine-specific configuration is bundled with the app.
