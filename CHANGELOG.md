# Changelog

## [0.2.2](https://github.com/tupe12334/rc-keeper/compare/v0.2.1...v0.2.2) (2026-11-03)


### Bug Fixes

* **install:** replace binaries through a rename ([#36](https://github.com/tupe12334/rc-keeper/issues/36)) ([d41855f](https://github.com/tupe12334/rc-keeper/commit/d41855ffe415ad33f4248431bad9e039c5ecaa92))

## [0.2.1](https://github.com/tupe12334/rc-keeper/compare/v0.2.0...v0.2.1) (2026-11-03)


### Bug Fixes

* **core:** clear stale errors and explain network failures ([#34](https://github.com/tupe12334/rc-keeper/issues/34)) ([3a04368](https://github.com/tupe12334/rc-keeper/commit/3a0436883cb94aade78fd9c5694ceb18644d16ef))

## [0.2.0](https://github.com/tupe12334/rc-keeper/compare/v0.1.1...v0.2.0) (2026-11-03)


### Features

* **gui:** setting to hide the Dock icon on macOS ([#33](https://github.com/tupe12334/rc-keeper/issues/33)) ([42b22f4](https://github.com/tupe12334/rc-keeper/commit/42b22f4f7bc81d63cc1e305550bf5ad267e465ae))


### Bug Fixes

* **ci:** regenerate docs on the release PR ([#30](https://github.com/tupe12334/rc-keeper/issues/30)) ([7de5d24](https://github.com/tupe12334/rc-keeper/commit/7de5d24827085f0e4798e9f948fad14879fda456))

## [0.1.1](https://github.com/tupe12334/rc-keeper/compare/v0.1.0...v0.1.1) (2026-11-02)


### Bug Fixes

* **ci:** upload Tauri bundles to the existing release by id ([#27](https://github.com/tupe12334/rc-keeper/issues/27)) ([0287a0d](https://github.com/tupe12334/rc-keeper/commit/0287a0d673a0619b1809f978c339074d7a00b783))

## 0.1.0 (2026-11-02)


### Features

* **brand:** app icon, tray template icon and favicon ([#6](https://github.com/tupe12334/rc-keeper/issues/6)) ([923dd4c](https://github.com/tupe12334/rc-keeper/commit/923dd4cc3451d49abb4d175875635d150adf48cf))
* **cli:** rc-keeper CLI and daemon with local IPC ([#4](https://github.com/tupe12334/rc-keeper/issues/4)) ([8c662b1](https://github.com/tupe12334/rc-keeper/commit/8c662b1aea3229318b8ca93647b7b98ebc43ba56))
* **core:** git-friendly config layout with live reload ([#8](https://github.com/tupe12334/rc-keeper/issues/8)) ([9ba4c4a](https://github.com/tupe12334/rc-keeper/commit/9ba4c4a8088ef8862da7d8385a0e5f51d27e7da6))
* force boot — take over a folder served by another claude rc ([#14](https://github.com/tupe12334/rc-keeper/issues/14)) ([5f7c8ce](https://github.com/tupe12334/rc-keeper/commit/5f7c8ceff2ad89ad3b85bf70ca9a8df64326ce82))
* **gui:** Tauri 2 desktop shell with tray and daemon sidecar ([#5](https://github.com/tupe12334/rc-keeper/issues/5)) ([c8b4ce8](https://github.com/tupe12334/rc-keeper/commit/c8b4ce806f64d3b51367458592863162a16f2b01))
* **rc-core:** PTY supervisor library ([#2](https://github.com/tupe12334/rc-keeper/issues/2)) ([9b09d65](https://github.com/tupe12334/rc-keeper/commit/9b09d650583ce7324771b6d1b2713f99ec7804d6))
* **ui:** info popovers with official docs links for every input ([#7](https://github.com/tupe12334/rc-keeper/issues/7)) ([c1ddce6](https://github.com/tupe12334/rc-keeper/commit/c1ddce61fd3efeec67ec0dcba945f7b6d3d960f4))
* **ui:** React frontend for rc-keeper ([#3](https://github.com/tupe12334/rc-keeper/issues/3)) ([4d63216](https://github.com/tupe12334/rc-keeper/commit/4d6321614c50b9e8dd6447ec1aad67f86804cf8c))


### Bug Fixes

* **cli:** auto-started daemon no longer holds caller stdout on Windows ([#19](https://github.com/tupe12334/rc-keeper/issues/19)) ([065cc77](https://github.com/tupe12334/rc-keeper/commit/065cc776b637c39c834cdffa21cd3d5375a7cc6d))
* **cli:** exit quietly when the output pipe closes early ([#23](https://github.com/tupe12334/rc-keeper/issues/23)) ([86bf7a5](https://github.com/tupe12334/rc-keeper/commit/86bf7a53badb78759ee965d5288b1124f9b08464))
* **cli:** service install hint on hosts without systemctl ([#21](https://github.com/tupe12334/rc-keeper/issues/21)) ([3461623](https://github.com/tupe12334/rc-keeper/commit/34616233b05ac454da9be1a190a17a950137f2ad))
* **core:** add starts an instance that has auto-start on ([#20](https://github.com/tupe12334/rc-keeper/issues/20)) ([83ee749](https://github.com/tupe12334/rc-keeper/commit/83ee749aef0b7140bce75410af418e6c38fb924e))
* cross-OS path and watcher bugs found by CI matrix ([#16](https://github.com/tupe12334/rc-keeper/issues/16)) ([4eaa000](https://github.com/tupe12334/rc-keeper/commit/4eaa0008de8f1dd69438096139a731b37080fcb7))
* **gui:** allow opening official docs and repo links ([#9](https://github.com/tupe12334/rc-keeper/issues/9)) ([4213207](https://github.com/tupe12334/rc-keeper/commit/42132078c5a951f175f437ba0310ed9463950724))
* **gui:** explain macOS folder access prompts ([#12](https://github.com/tupe12334/rc-keeper/issues/12)) ([8757f47](https://github.com/tupe12334/rc-keeper/commit/8757f472a31037e5a07172377d0386ff7afd4165))
* **ipc:** socket in short per-user runtime dir ([#15](https://github.com/tupe12334/rc-keeper/issues/15)) ([2df3698](https://github.com/tupe12334/rc-keeper/commit/2df3698177b3b573ae4bb4a0f3d3ed8c817d2880))
* Windows build of rc-ipc and stale src-tauri lock ([#18](https://github.com/tupe12334/rc-keeper/issues/18)) ([abaa4e6](https://github.com/tupe12334/rc-keeper/commit/abaa4e6e8f998b23cb576a984c0d08886f10a339))

## Changelog
