---
links:
  '#61': https://github.com/dahlia/gukhanmun/issues/61
  '#75': https://github.com/dahlia/gukhanmun/pull/75
---
 -  Added `--generate-completion` for Bash, zsh, fish, PowerShell and Nushell,
    with option, value and file candidates derived from the CLI definition.
    Platform binary archives now include matching completion scripts and their
    source revision, version and SHA-256 metadata. Bundle generation rejects a
    checkout revision change during the build. Bundle metadata records source
    changes present before or after generation, excluding only untracked files
    created by the bundle generator. [[#61], [#75]]
