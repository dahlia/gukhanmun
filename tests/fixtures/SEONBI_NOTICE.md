Seonbi fixture provenance
=========================

The following Gukhanmun regression fixtures originate from
[Seonbi]'s `test/data/` directory:

| Gukhanmun stem           | Seonbi source                    |
| ------------------------ | -------------------------------- |
| `html/initial-sound-raw` | `initial-sound-raw.ko-Kore.html` |
| `html/seup-gwan-eum`     | `習慣音.ko-Kore.html`            |
| `html/preservation`      | `preservation.ko-Kore.html`      |
| `html/i-reon-nal`        | `이런날.ko-Kore.html`            |

Each `*.input.html` is the corresponding Seonbi `*.ko-Kore.html` reproduced
byte-for-byte.  Each `*.expected.html` is *re-derived* for Gukhanmun's narrower
scope: Gukhanmun performs only hanja-to-hangul conversion and does not apply
the typographic adjustments (smart quotes, dashes, ellipses, word spacing)
that Seonbi additionally runs on its corpora.  The expected outputs therefore
diverge from Seonbi's `*.ko-KR.html` / `*.ko-KP.html` whenever the original
also contained a typographic transformation.

Seonbi is licensed under the GNU General Public License version 3 or later,
identical to Gukhanmun's own license, so reproducing these inputs poses no
compatibility issue.  The sidecar `*.toml` file for each fixture records the
preset and any dictionary overrides used to drive the regression assertion.

Seonbi's `大韓民國憲法第十號前文.ko-Kore.html` corpus is intentionally not
ported in the initial batch.  Its `ko-KR` and `ko-KP` reference outputs lean
heavily on Seonbi's automatic word-spacing pass — a behaviour Gukhanmun does
not provide — so a faithful expected file requires hand-derivation against
the bundled *Standard Korean Language Dictionary*.  It is tracked as a
follow-up regression once the dictionary-driven expected output can be
generated reproducibly.

[Seonbi]: https://github.com/dahlia/seonbi
