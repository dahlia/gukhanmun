---
title: 支援 플랫폼
description: Gukhanmun Python 휠이 支援하는 플랫폼과 인터프리터, 最小 버전, 그리고 原始 코드 配布版에서 패키지를 빌드하는 方法을 說明합니다.
---

支援 플랫폼
===========

`gukhanmun` 패키지는 널리 쓰이는 플랫폼을 爲한 휠(wheel)과 原始 코드
配布版(sdist)으로 提供됩니다. 휠은 Rust 툴체인 없이 設置되며, 그 밖의
플랫폼에서는 原始 코드 配布版으로 패키지를 빌드합니다.

이 패키지들은 아직 PyPI에 配布되지 않았습니다. 配布되기 前까지는
〈[Python 빠른 始作](./start.md#로컬-휠wheel-빌드와-設置)〉에서 說明한 대로
로컬에서 휠을 빌드합니다.


휠
--

| 플랫폼        | 아키텍처         | 最小 버전                     | CPython   | PyPy       |
| ------------- | ---------------- | ----------------------------- | --------- | ---------- |
| Linux (glibc) | x86\_64, aarch64 | glibc 2.28 (`manylinux_2_28`) | 3.11 以上 | 3.11, 3.12 |
| Linux (musl)  | x86\_64, aarch64 | musl 1.2 (`musllinux_1_2`)    | 3.11 以上 |            |
| macOS         | x86\_64, arm64   | macOS 11                      | 3.11 以上 | 3.11, 3.12 |
| Windows       | AMD64            | Windows 10                    | 3.11 以上 | 3.11, 3.12 |
| Windows       | ARM64            | Windows 10                    | 3.11 以上 |            |

CPython用으로는 플랫폼마다 Python 3.11의 安定 ABI(`abi3`)에 맞춰 빌드한 휠이
하나씩 있으며, 이 휠은 그 뒤의 모든 CPython 버전에서도 實行됩니다. 各 휠은
該當 運營 體制와 아키텍처에서 支援하는 가장 낮은 버전과 가장 높은 버전의
CPython으로 設置하여 檢査합니다. glibc 휠은 glibc 2.28 自體에서도, musl 휠은
musl 시스템에서도 實行합니다.

Windows 10은 擴張 모듈을 빌드하는 Rust 툴체인의 最小 要件이며, 使用하는 Python
인터프리터도 該當 Windows 버전을 支援해야 합니다. 擴張 모듈은 CPython이 함께
提供하는 Visual C++ 런타임을 使用합니다.

### PyPy

PyPy 휠은 PyPy 8.0의 Python 3.11 버전과 Python 3.12 버전用으로 빌드합니다.
CPython 휠과 달리 PyPy 휠은 빌드할 때의 PyPy ABI(`pypy311_pp80`,
`pypy312_pp80`)에만 맞습니다. ABI가 다른 PyPy 릴리스에서는 原始 코드
配布版으로 패키지를 빌드합니다.

musl과 Windows ARM64用 PyPy는 없으므로, 이 플랫폼에는 PyPy 휠도 없습니다.

### 支援하지 않는 인터프리터

`python3.14t`와 같은 自由 스레드(free-threaded) CPython 빌드는 支援하지
않습니다. `abi3` 휠은 이를 包含하지 않으며, 自由 스레드用 휠은 빌드하지도
檢査하지도 않습니다. 32비트 플랫폼도 支援하지 않습니다.


原始 코드 配布版에서 빌드
-------------------------

휠이 없는 플랫폼에서, 또는 明示的으로 要請하면 pip는 原始 코드 配布版으로
패키지를 빌드합니다. 패키지가 配布되기 前까지는 체크아웃에서 原始 코드 配布版을
빌드하여 그 아카이브를 設置합니다. `0.4.0`은 빌드한 버전으로 바꿉니다.

~~~~ sh
mise run py-sdist
pip install target/python/sdist/gukhanmun-0.4.0.tar.gz
~~~~

패키지가 PyPI에 配布된 뒤에는 `pip install --no-binary gukhanmun gukhanmun`이
配布된 原始 코드 配布版을 빌드합니다.

빌드에는 다음이 必要합니다.

 -  Rust 1.95 以上. 例를 들어 [rustup]으로 設置합니다.
 -  플랫폼의 C 툴체인과 링커. Windows에서는 Visual Studio Build Tools,
    macOS에서는 Xcode Command Line Tools, Linux에서는 C 컴파일러입니다.
 -  crates.io에 接續할 수 있는 네트워크. Cargo는 配布版의 잠금 파일에 記錄된
    버전대로 Rust 依存性을 내려받습니다.

빌드 백엔드인 maturin은 pip가 스스로 設置합니다. 原始 코드 配布版에는 빌드에
必要한 모든 原始 코드가 들어 있으므로, Gukhanmun 貯藏所 체크아웃이나 Git,
辭典 原本 데이터는 必要하지 않습니다. 辭典 데이터 配布版은 플랫폼 獨立的 휠을
提供하는 純粹 Python 패키지이며, 그 原始 코드 配布版에도 데이터가 이미 들어
있습니다.

[rustup]: https://rustup.rs/
