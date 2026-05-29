---
title: "HTML processing"
description: |-
  How Gukhanmun handles HTML input and which elements it always skips.
---

HTML processing
===============

Pass `-f text/html` (or use a *.html*/*.htm* extension) to convert an HTML
document or fragment.  Gukhanmun parses the HTML, converts hanja in text
nodes and attributes, and serialises the result back to HTML while preserving
all tags and attributes.


Elements that are always preserved
----------------------------------

Gukhanmun never modifies content inside these elements regardless of any
other settings:

 -  `<code>`, `<kbd>`, `<pre>`, `<samp>` — code and preformatted text
 -  `<script>`, `<style>` — scripts and stylesheets
 -  `<textarea>` — user input areas
 -  Elements with `translate="no"` — explicit opt-out

Content inside `<ruby>` annotations is also left as-is.


Preserving additional elements by CSS class
-------------------------------------------

Use `--html-preserve-class` to skip conversion inside elements that carry a
specific class.  The flag can be repeated:

~~~~ sh
gukhanmun -f text/html \
  --html-preserve-class math \
  --html-preserve-class no-translate \
  input.html
~~~~

Any element with one of those classes (and all of its descendants) is passed
through unchanged.


Preserving elements by attribute
--------------------------------

Use `--html-preserve-attr` to skip conversion based on an attribute name alone
or an `attribute=value` pair.  The flag can be repeated:

~~~~ sh
gukhanmun -f text/html \
  --html-preserve-attr data-no-hanja \
  --html-preserve-attr lang=en \
  input.html
~~~~

The first form matches any element that has the attribute (regardless of its
value).  The second form matches only elements whose attribute equals the
given value.


Ruby markup
-----------

Combine `-f text/html` with `--rendering ruby-on-hangul` or
`--rendering ruby-on-hanja` to wrap conversions in `<ruby>` elements:

~~~~ sh
echo "<p>漢字</p>" | gukhanmun -f text/html --rendering ruby-on-hangul
# → <p><ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby></p>
~~~~

The annotation is wrapped in `<rp>` (ruby parenthesis) elements.  Browsers
that understand `<ruby>` hide the `<rp>` content and render the reading
stacked above the base; browsers without `<ruby>` support fall back to
showing the parenthesized gloss inline (`한자(漢字)`), which keeps the
output readable everywhere.
