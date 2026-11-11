---
title: 公式 辭典
description: Python extra로 《標準國語大辭典》, 《우리말샘》과 國立國語院 《外來語 表記 用例》 데이터를 設置하고, gukhanmun.dictionaries로 明示的으로 活性化하는 方法을 說明합니다.
---

公式 辭典
=========

公式 辭典은 `gukhanmun` 패키지와 別個인 選擇的 데이터 配布版으로 提供됩니다.
extra로 데이터를 設置하고, `gukhanmun.dictionaries`의 函數로 變換器에
追加합니다. 데이터를 設置하는 것만으로는 變換器가 달라지지 않으며, 어느
프리셋도 이 데이터를 불러오지 않습니다.

~~~~ python
from gukhanmun import Converter
from gukhanmun.dictionaries import stdict

converter = Converter(dictionaries=[stdict()])

assert converter.convert("無條件降服") == "무조건항복"
~~~~


데이터 設置
-----------

各 辭典의 extra는 `gukhanmun`과 正確히 같은 버전의 데이터를 設置합니다.

| extra                        | 配布版                    | 데이터                               |
| ---------------------------- | ------------------------- | ------------------------------------ |
| `stdict`, `stdict-fst`       | `gukhanmun-stdict-fst`    | 《標準國語大辭典》, FST              |
| `stdict-cdb`                 | `gukhanmun-stdict-cdb`    | 《標準國語大辭典》, CDB              |
| `opendict`, `opendict-fst`   | `gukhanmun-opendict-fst`  | 《우리말샘》, FST                    |
| `opendict-cdb`               | `gukhanmun-opendict-cdb`  | 《우리말샘》, CDB                    |
| `loanwords`, `loanwords-fst` | `gukhanmun-loanwords-fst` | 國立國語院 《外來語 表記 用例》, FST |
| `loanwords-cdb`              | `gukhanmun-loanwords-cdb` | 國立國語院 《外來語 表記 用例》, CDB |

~~~~ sh
pip install 'gukhanmun[stdict,loanwords]'
~~~~

데이터 配布版은 依存性이 없는 플랫폼 獨立的 wheel입니다. `gukhanmun`을
가져오지 않으므로, `gukhanmun`만 設置하면 公式 데이터는 包含되지 않습니다.

이 패키지들은 아직 PyPI에 配布되지 않았습니다. 配布되기 前까지는 貯藏所를
체크아웃하여 빌드하고, 로컬 産出物에서 設置합니다.

~~~~ sh
mise run py-build
mise run py-dicts-build
pip install --find-links target/python/wheels \
            --find-links target/python/dictionaries/gukhanmun-stdict-fst \
            'gukhanmun[stdict]==0.4.0'
~~~~

`0.4.0`은 빌드한 버전으로 바꿉니다. *target/python/wheels/* 디렉터리에는 빌드한
플랫폼用 `gukhanmun` wheel만 들어 있습니다. 設置할 데이터 配布版마다
`--find-links`를 하나씩 追加합니다.


變換器에 辭典 追加
------------------

`gukhanmun.dictionaries`는 辭典마다 函數를 하나씩 提供합니다.

| 函數                      | 데이터                          | 出處 役割   |
| ------------------------- | ------------------------------- | ----------- |
| `stdict()`                | 《標準國語大辭典》              | `REFERENCE` |
| `opendict_general()`      | 《우리말샘》, 一般語            | `REFERENCE` |
| `opendict_north_korean()` | 《우리말샘》, 北韓語            | `REFERENCE` |
| `opendict_dialect()`      | 《우리말샘》, 方言              | `REFERENCE` |
| `opendict_archaic()`      | 《우리말샘》, 옛말              | `REFERENCE` |
| `loanwords()`             | 國立國語院 《外來語 表記 用例》 | `LOANWORD`  |

各 函數는 辭典 바이트, 形式, 出處 役割을 담은 `FileDictionarySource`를
返還합니다. 《우리말샘》의 네 分類는 各各 別個의 辭典이므로, 必要한 것만
追加합니다.

~~~~ python
from gukhanmun import Converter, Preset
from gukhanmun.dictionaries import opendict_general, opendict_north_korean

converter = Converter(
    preset=Preset.KO_KP,
    dictionaries=[opendict_north_korean(), opendict_general()],
)
~~~~

出處 役割은 辭典들이 協力하는 方式을 決定합니다. 基本 政策인
`SourcePolicy.REFERENCE_AUTHORIZED`에서는 參照 辭典이 立證한 外來語 讀音이
順序와 無關하게 優先합니다.

~~~~ python
from gukhanmun.dictionaries import loanwords, stdict

converter = Converter(dictionaries=[stdict(), loanwords()])
assert converter.convert("香港") == "홍콩"
~~~~

다른 出處 政策과 `reference_only`에 對해서는
〈[API 參考 文書](../../api/python.md#辭典-入力과-項目)〉를 參照하십시오.

變換器는 生成될 때 데이터를 한 番 읽습니다. 文書마다 函數를 呼出하지 말고
變換器를 再使用하십시오.


形式 選擇
---------

모든 函數는 키워드 專用 引數 `format`을 받으며, 基本값은
`DictionaryFormat.FST`입니다.

~~~~ python
from gukhanmun import DictionaryFormat
from gukhanmun.dictionaries import stdict

converter = Converter(dictionaries=[stdict(format=DictionaryFormat.CDB)])
~~~~

두 形式은 같은 데이터를 담으며, 變換 結果도 같습니다. FST 데이터는 크기가
더 작고, CDB는 常數 時間 照會를 提供합니다. 使用할 形式의 extra를
設置하십시오.

《標準國語大辭典》과 《우리말샘》의 CDB 配布版은 파일을 gzip으로 壓縮하여
貯藏합니다. 函數가 壓縮을 풀므로, 返還된 바이트는 恒常 辭典 로더에서 바로
使用할 수 있습니다.


바이트 읽기
-----------

各 函數에는 `stdict_bytes()`처럼 對應하는 `*_bytes()` 函數가 있습니다. 이
函數는 壓縮을 푼 辭典 바이트만 返還하므로, 캐시하거나 다른 役割을 指定하여
넘길 때 使用할 수 있습니다.

~~~~ python
from gukhanmun import DictionaryFormat, FileDictionarySource, SourceRole
from gukhanmun.dictionaries import stdict_bytes

source = FileDictionarySource(stdict_bytes(), DictionaryFormat.FST, SourceRole.OVERRIDE)
~~~~

패키지에 包含된 파일은 `importlib.resources`로도 읽을 수 있습니다. 各
配布版의 `import`用 패키지에는 辭典 파일과, 原本 스냅숏 및 파일 해시를
記錄한 *provenance.json*이 들어 있습니다.

~~~~ python
from importlib.resources import files

data = (files("gukhanmun_stdict_cdb") / "stdict.cdb.gz").read_bytes()
~~~~

이 方法으로 읽은 *.gz* 파일은 如前히 壓縮된 狀態입니다.


데이터 缺落 處理
----------------

데이터 配布版이 設置되어 있지 않으면, 函數는 `ErrorCode.DICTIONARY_LOAD`인
`GukhanmunError`를 發生시킵니다. 메시지에는 데이터를 設置하는 extra가
나타납니다.

~~~~ python
from gukhanmun import ErrorCode, GukhanmunError
from gukhanmun.dictionaries import stdict

try:
    source = stdict()
except GukhanmunError as error:
    assert error.code is ErrorCode.DICTIONARY_LOAD
    print(error.message)
    # Standard Korean Language Dictionary FST data is not installed;
    # install it with: pip install 'gukhanmun[stdict]'
~~~~

한 形式이 없어도 다른 形式은 使用할 수 있습니다. 設置된 配布版이
損傷되었으면, 例를 들어 파일이 없거나 壓縮된 파일이 損傷되었으면, 誤謬
메시지가 `pip install --force-reinstall`로 再設置할 것을 案內하며,
`error.chain`에 根本 原因이 남습니다. 그 밖에 損傷된 辭典 內容은 나중에
`Converter(...)`가 불러올 때 誤謬가 됩니다.


라이선스
--------

데이터 配布版에는 各 데이터의 라이선스가 適用됩니다.

 -  《標準國語大辭典》과 《우리말샘》 데이터: 크리에이티브 커먼즈
    著作者表示-同一條件變更許諾 2.0 大韓民國 (CC BY-SA 2.0 KR).
 -  國立國語院 《外來語 表記 用例》: 國立國語院의 自由 利用 條件.

各 配布版에는 出處 表示와 코드 라이선스를 담은 *LICENSE*와 *ATTRIBUTION.md*가
包含됩니다. 데이터를 再配布할 때는 이 파일들을 維持하십시오.
