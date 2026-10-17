---
links:
  '#11': https://github.com/dahlia/gukhanmun/issues/11
  '#12': https://github.com/dahlia/gukhanmun/pull/12
---
 -  Added `Builder::hanja_variant_set()` for selecting the hanja spelling used
    in rendered output independently of the rendering mode.  A separately
    selected variant set remains in effect when the rendering mode changes,
    while complete `RenderOptions` replace both settings.  [[#11], [#12]]
