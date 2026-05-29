---
title: Markdown 處理
description: |-
  Gukhanmun이 Markdown 文書를 變換하는 方式과 選擇된 YAML 프런트 매터 필드.
---

Markdown 處理
=============

Markdown 文書를 變換하려면 `-f text/markdown`을 넘기거나 *.md*/*.markdown*
擴張子를 使用합니다.  Gukhanmun은 Markdown 本文 속의 漢字를 變換하고, 自身이
干涉하지 않는 構文은 그대로 둡니다.  GitHub Flavored Markdown(表, 脚註, 取消線,
할 일 目錄)을 켜려면 `variant` 媒介變數를 더합니다:

~~~~ sh
gukhanmun -f "text/markdown; variant=GFM" input.md
~~~~


YAML 프런트 매터
----------------

파일 맨 위의 YAML 프런트 매터(`---`로 구분된 블록)는 變換 前에 恒常 認識되어
分離되므로, Markdown 變換器에 依해 망가지지 않습니다.  基本的으로는 그대로
通過되고 Markdown 本文만 變換됩니다.

選擇된 프런트 매터 값까지 함께 變換하려면 `--markdown-frontmatter-convert`를
使用하여 [JSONPath] 式으로 그 값을 가리킵니다.  이 플래그는 反復할 수 있습니다:

~~~~ sh
gukhanmun -f text/markdown \
  --markdown-frontmatter-convert '$.hero.tagline' \
  --markdown-frontmatter-convert '$.hero.actions[*].text' \
  page.md
~~~~

選擇子에 一致하는 各 값은 國漢文 混用體에서 한글로 變換됩니다.  文字列
스칼라만 變換되며, 數値나 眞僞值 等 文字列이 아닌 一致 對象과 어떤 選擇子도
가리키지 않는 필드는 그대로 둡니다.

> [!NOTE]
> 選擇子가 없으면 프런트 매터는 바이트 單位로 그대로 保存됩니다.  選擇子가
> 하나라도 주어지면 블록을 解析한 뒤 다시 直列化하므로, 一致한 값만 바뀌더라도
> 블록 全體가 出力에서 다시 整形됩니다(註釋이 사라지고 引用 方式이 바뀔 수
> 있습니다).  닫는 울타리가 없는 맨 앞의 `---`는 普通 Markdown으로 남습니다.  이
> 플래그는 `--format text/markdown`에서만 有效합니다.

[JSONPath]: https://www.rfc-editor.org/rfc/rfc9535
