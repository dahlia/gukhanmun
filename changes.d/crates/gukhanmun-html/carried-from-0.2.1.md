 -  Escaped dictionary readings and other annotation text when writing HTML,
    including quotes and a guarded first character, so a reading can no longer
    become an element or an attribute.  [[GHSA-j87h-8h88-wm6g]]
 -  Stopped treating `<script/>`, `<style/>`, and `<textarea/>` as self-closing
    so their contents stay raw text, matching browsers.  Text after a
    self-closed raw-text tag inside SVG or MathML is now left unconverted
    rather than risking conversion inside a script or style body.
    [[GHSA-j87h-8h88-wm6g]]

[GHSA-j87h-8h88-wm6g]: https://github.com/dahlia/gukhanmun/security/advisories/GHSA-j87h-8h88-wm6g
