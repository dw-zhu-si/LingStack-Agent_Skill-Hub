# App Store marketing assets

The screenshots in this directory are generated from an isolated demo home containing synthetic Agents, Skills, projects, and model profiles. The fixture is not compiled into or shipped with LingStack.

Generate a fresh isolated fixture with:

```sh
node marketing/generate-demo-home.mjs /absolute/path/to/temporary/home
```

Never capture screenshots against a real user home directory. Final App Store images must be 2880×1800 RGB PNG files with no alpha channel.

The checked-in `store-preview/output/zh-CN/` set contains eight final promotional screenshots. Raw window captures are intentionally ignored because they are intermediate fixture output; regenerate them from a fresh isolated demo home before rerendering. Verify final files with `store-preview/SHA256SUMS`.
