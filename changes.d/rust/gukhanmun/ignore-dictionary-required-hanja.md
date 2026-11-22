---
links:
  '#81': https://github.com/dahlia/gukhanmun/issues/81
  '#86': https://github.com/dahlia/gukhanmun/pull/86
---
 -  Added `.ignore_dictionary_required_hanja(true)` to `Builder` and
    `SharedBuilder` to omit hanja glosses required only by dictionary entries
    in `RenderMode::HangulOnly` across text, HTML, and Markdown. Homophone
    glosses and explicit annotations remain visible. [[#81], [#86]]
