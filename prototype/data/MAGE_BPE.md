# MAGE BPE vocabulary

`mage_bpe_4096.tiktoken` is a byte-level BPE vocabulary (4096 tokens: the 256
bytes plus 3840 merges) trained by this crate's own trainer (`src/bpe.rs`) on the
repo's `.mg` sources. It is compiled into `mage-parse` and is the default for
`--bpe-count`. It is a MAGE-BPE vocabulary, not cl100k: counts from it are not
comparable to cl100k counts.

Format: tiktoken text, one `<base64 of token bytes> <rank>` per line.

## Corpus

`mage_bpe_4096.corpus.txt` lists the corpus, one repo-relative path per line:
every git-tracked `*.mg` file at training time (127 files), sorted bytewise.
Line endings are normalised CRLF to LF by the trainer, so the result does not
depend on how git checked the files out.

## Regenerate

From the repo root, after building `mage-parse`:

    mage-parse --bpe-train --vocab-size 4096 --out prototype/data/mage_bpe_4096.tiktoken $(cat prototype/data/mage_bpe_4096.corpus.txt)

Training is deterministic (a release and a debug build produce identical bytes).
`--bpe-train` prints the corpus digest; record it in `mage_bpe_4096.corpus.sha256`.

`bpe::tests::shipped_vocab_is_reproduced_by_retraining` retrains from the corpus list
and fails if the shipped file differs, but only while the listed files still hash to
the recorded digest. Editing any listed `.mg` file therefore does not fail the build;
it just stops the strict check until the vocabulary is regenerated. Refresh the list
with `git ls-files '*.mg' | LC_ALL=C sort` if files were added.
