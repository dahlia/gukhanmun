---
title: 辭典
description: Python에서 公式 辭典, 메모리 內 項目과 컴파일된 FST/CDB 辭典을 불러오고, 出處 役割과 選擇 政策을 設定하는 方法입니다.
---

辭典
====

프로그램의 語彙는 메모리 內 項目으로 定義하거나, 컴파일된 FST/CDB 辭典으로
불러옵니다. 公式 辭典은 `gukhanmun` 패키지와 別個인 選擇的 데이터 配布版으로
提供됩니다. 엑스트라로 데이터를 設置하고, `gukhanmun.dictionaries`의 函數로
變換器에 追加합니다. 데이터를 設置하는 것만으로는 變換器가 달라지지 않으며,
어느 프리셋도 이 데이터를 불러오지 않습니다.

~~~~ python
from gukhanmun import Converter
from gukhanmun.dictionaries import stdict

converter = Converter(dictionaries=[stdict()])

assert converter.convert("無條件降服") == "무조건항복"
~~~~


데이터 設置
-----------

各 辭典의 엑스트라는 `gukhanmun`과 正確히 같은 버전의 데이터를 設置합니다.

| 엑스트라                     | 配布版                    | 데이터                               |
| ---------------------------- | ------------------------- | ------------------------------------ |
| `stdict`, `stdict-fst`       | `gukhanmun-stdict-fst`    | 《標準國語大辭典》, FST              |
| `stdict-cdb`                 | `gukhanmun-stdict-cdb`    | 《標準國語大辭典》, CDB              |
| `opendict`, `opendict-fst`   | `gukhanmun-opendict-fst`  | 《우리말샘》, FST                    |
| `opendict-cdb`               | `gukhanmun-opendict-cdb`  | 《우리말샘》, CDB                    |
| `loanwords`, `loanwords-fst` | `gukhanmun-loanwords-fst` | 國立國語院 《外來語 表記 用例》, FST |
| `loanwords-cdb`              | `gukhanmun-loanwords-cdb` | 國立國語院 《外來語 表記 用例》, CDB |

데이터 配布版은 依存性이 없는 플랫폼 獨立的 휠(wheel)입니다. `gukhanmun`을
가져오지 않으므로, `gukhanmun`만 設置하면 公式 데이터는 包含되지 않습니다.

〈[設置](./install.mdx#公式-辭典)〉에 따라 PyPI의 엑스트라를 設置합니다.
例를 들어 `gukhanmun[stdict,loanwords]`를 使用할 수 있습니다. 設置 文書에는
pip·uv·Poetry 命令과 프리릴리스(pre-release)·로컬 빌드 使用法이 있습니다.


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

出處 役割은 辭典들이 協力하는 方式을 決定합니다. 各 函數가 出處마다 役割을
指定하므로, 基本 政策인 `SourcePolicy.REFERENCE_AUTHORIZED`에서는 外來語 出處를
參照 出處 앞에 두든 뒤에 두든 어느 쪽이 優先하는지가 달라지지 않습니다. 다음
例에서는 參照 辭典이 外來語 讀音을 立證합니다.

~~~~ python
from gukhanmun.dictionaries import loanwords, stdict

converter = Converter(dictionaries=[stdict(), loanwords()])
assert converter.convert("香港") == "홍콩"
~~~~

《우리말샘》의 分類처럼 參照 出處가 여럿이면, 그 出處들이 서로 다른 讀音을
提供할 때는 如前히 登錄 順序가 決定합니다. `OVERRIDE` 出處, 다른 出處 政策과
`reference_only`에 對해서는
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
더 작고, CDB는 常數 時間 照會를 提供합니다. 使用할 形式의 엑스트라를
設置하십시오.

《標準國語大辭典》과 《우리말샘》의 CDB 配布版은 파일을 gzip으로 壓縮하여
貯藏합니다. 函數가 壓縮을 풀므로, 返還된 바이트는 恒常 辭典 로더(loader)에서
바로 使用할 수 있습니다.


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
`GukhanmunError`를 發生시킵니다. 메시지에는 데이터를 設置하는 엑스트라가
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
損傷되었으면, 例를 들어 파일이 없거나 *provenance.json*에 記錄된 크기 및
해시와 다르면, 誤謬 메시지가 `pip install --force-reinstall`로 再設置할 것을
案內하며, 根本 原因이 있으면 `error.chain`에 남습니다.

案內하는 命令은 PyPI를 使用합니다. 다른 패키지 管理者나 로컬 빌드는
〈[設置](./install.mdx)〉를 參照하십시오.


實行 中의 辭典 項目
-------------------

애플리케이션 語彙에는 `MapDictionarySource`를 使用합니다. 項目
시퀀스(sequence)는 反復되는 漢字 表記를 하나의 對應으로 合치는 代身 競爭하는
讀音으로 維持합니다.

~~~~ python
from gukhanmun import ContextWindow, Converter, DictionaryEntry, MapDictionarySource

source = MapDictionarySource([
    DictionaryEntry("東京", "동경"),
    DictionaryEntry("東京", "도쿄", preference=10),
    DictionaryEntry("年代", "연대", suffix_reading="년대"),
    DictionaryEntry("庫間", "곳간", require_hanja=True),
])
converter = Converter(dictionaries=[source], homophone_window=ContextWindow.OFF)
assert converter.convert("東京") == "도쿄"
assert converter.convert("東京을") == "동경을"
assert converter.convert("東京를") == "도쿄를"
assert converter.convert("1990年代 年代") == "1990년대 연대"
assert converter.convert("庫間") == "곳간(庫間)"
~~~~

基本的으로 더 높은 `preference`가 優先하지만, 뒤따르는 韓國語 助詞가
代身 이에 맞는 候補를 選擇할 수 있습니다. 例示의 `東京을`에서는 `을`이
`동경`을 選擇합니다. `suffix_reading`은 單語 첫머리가 아닐 때,
例를 들어 數詞 뒤에 使用할 讀音을 提供합니다. 項目의 `require_hanja`와
`require_hangul`은 別途의 [指示](./directives.md)처럼 註解(annotation) 標識를
要求합니다.

`MapDictionarySource`는 項目을 變更할 수 없는 튜플(tuple)로 複寫합니다.
나중에 原本 목록을 바꾸어도 設定된 出處(source)나 變換器는 바뀌지 않습니다.


出處 役割 結合
--------------

맵 出處의 基本 役割은 `SourceRole.OVERRIDE`입니다. 使用者 덮어쓰기(override)는
같은 表記와 消費 길이의 協力 參考/外來語 候補를 代替하며, 앞의 덮어쓰기
出處가 優先합니다. 덮어쓰기를 公式 出處와 明示的으로 結合할 수 있습니다.

~~~~ python
from gukhanmun.dictionaries import loanwords, stdict

converter = Converter(dictionaries=[stdict(), loanwords(), source])
~~~~

이 例題는 公式 엑스트라(extra)가 設置되어 있어야 합니다. 맵 出處는 마지막에
있어도 一致하는 協力 候補를 덮어씁니다.

`source_policy`는 出處 사이의 協力을 制御합니다.

| `SourcePolicy`                | 選擇                                                       |
| ----------------------------- | ---------------------------------------------------------- |
| `REFERENCE_AUTHORIZED` (基本) | 參考 根據로 代替 表記를 許容함                             |
| `REFERENCE_FIRST`             | 參考 候補를 優先함                                         |
| `LOANWORD_FIRST`              | 外來語 候補를 優先함                                       |
| `ORDERED`                     | 役割과 `reference_only`를 無視하고 出處 登錄 順序를 使用함 |

`ORDERED`를 뺀 세 協力 政策에서 `reference_only=True`는 獨立的인 外來語
選擇을 除外하되,
參考 根據로 許容한 表記는 維持합니다. 出處 役割만으로는 表記를 許容하는 데
必要한 語彙 根據가 생기지 않습니다.


컴파일된 辭典과 遠隔 辭典
-------------------------

`FileDictionarySource`는 原始 바이트나 파일시스템(filesystem) 經路를
明示的 `DictionaryFormat.FST` 또는 `CDB`와 함께 받습니다.
`UrlDictionarySource`는 變換器 生成 中 明示的 HTTP(S) URL을 불러옵니다.
文字列 經路를 URL로 推測하지는 않습니다. 둘 다 한 番 불러오며, 變換 呼出은
辭典 入出力을 遂行하지 않습니다.

바이너리 出處의 役割은 明示的 役割, 파일에 內藏된 役割 메타데이터,
`OVERRIDE` 順으로
定합니다. 파일名은 役割을 決定하지 않습니다. URL 오리진(origin) 制限,
타임아웃(timeout), 다운로드 制限은
[API 參考 文書](../../api/python.md#辭典-入力과-項目)를 參照하십시오.

큰 使用者 定義 辭典을 컴파일하려면
[辭典 빌드 案內](../cli/dictionary.md#使用者-定義-辭典-構築)를 따릅니다. 生成된
FST/CDB 파일은 위의 Python 出處와 함께 使用할 수 있습니다.


라이선스
--------

데이터 配布版에는 各 데이터의 라이선스가 適用됩니다.

 -  《標準國語大辭典》과 《우리말샘》 데이터: 크리에이티브 커먼즈
    著作者表示-同一條件變更許諾 2.0 大韓民國 (CC BY-SA 2.0 KR).
 -  國立國語院 《外來語 表記 用例》: 國立國語院의 自由 利用 條件.

各 配布版에는 出處 表示와 코드 라이선스를 담은 *LICENSE*와 *ATTRIBUTION.md*가
包含됩니다. 데이터를 再配布할 때는 이 파일들을 維持하십시오.
