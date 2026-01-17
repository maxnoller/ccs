# Changelog

## [0.2.0](https://github.com/maxnoller/ccs/compare/v0.1.0...v0.2.0) (2026-01-17)


### Features

* add --dry-run flag to preview docker command without executing ([c108233](https://github.com/maxnoller/ccs/commit/c108233f51bbc2d8f8edf4565463218dfc6953af))
* add detached mode and session management ([63882c9](https://github.com/maxnoller/ccs/commit/63882c9b20cd0b0084c025cf185aecb458be8775))
* add lazy worktree cleanup and toolchain auto-detection ([8effd61](https://github.com/maxnoller/ccs/commit/8effd61cd7bd195da977a1d48e38f9b3bc270dfe))
* add OAuth credential discovery for Claude Max ([02b29c1](https://github.com/maxnoller/ccs/commit/02b29c1bf6fa17f767977148f311110127c60394))
* add shell completions via --completions flag ([cbd281e](https://github.com/maxnoller/ccs/commit/cbd281e2d98b0ee4e7d66a6600540c8e22d7bd7a)), closes [#9](https://github.com/maxnoller/ccs/issues/9)
* add shell completions via --completions flag ([#9](https://github.com/maxnoller/ccs/issues/9)) ([14e89c2](https://github.com/maxnoller/ccs/commit/14e89c24e83a00af4d475a5b0c5f899b3ec72a81))
* default to worktree mode with XDG data directory ([8db1ba5](https://github.com/maxnoller/ccs/commit/8db1ba5bc7606ffae90f34aa715f40b05614ab90))
* default to worktree mode with XDG data directory ([#2](https://github.com/maxnoller/ccs/issues/2)) ([f4d8e9d](https://github.com/maxnoller/ccs/commit/f4d8e9da52172b808ae74c8f0448e869006c0961))


### Bug Fixes

* correct release-please config for patch-only bumps ([e55b30a](https://github.com/maxnoller/ccs/commit/e55b30a8978c749afac484623d0a9d6a50a1a441))
* make MacOsKeychain test conditional on macOS ([70065e3](https://github.com/maxnoller/ccs/commit/70065e33bafebf6510ccb0db8246740dd1d1e4ed))
* reuse existing node user in docker image ([93ceb67](https://github.com/maxnoller/ccs/commit/93ceb67a20121ca401fa89f49176896feb210c6d))


### Performance Improvements

* **mcp:** optimize args allocation by removing unnecessary clone ([#3](https://github.com/maxnoller/ccs/issues/3)) ([ddbeb78](https://github.com/maxnoller/ccs/commit/ddbeb785a003c6dde2137a17d0bad70001255c64))
* optimize container name selection to avoid redundant clones ([#6](https://github.com/maxnoller/ccs/issues/6)) ([95db886](https://github.com/maxnoller/ccs/commit/95db886a7c046e859bb45be91da85efc4464367e))
