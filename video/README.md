# `video/` — the Agentic Digital Rain render

A small [Remotion](https://remotion.dev) (React) project that renders
`out/agentic-rain.mp4`: a Matrix-style digital-rain animation whose falling
glyphs are **real MAGE agent-mode sigils** — `+f`, `~>`, `?=`, `@@`, `af`, `&m`,
`>>` — not decorative noise.

It exists because of the dense-UTF-8 "digital rain" representation of MAGE
(roadmap step 105, `prototype/src/rain.rs`, commit *"Add 'digital rain' — a
Matrix-inspired dense-UTF-8 representation of MechGen"*). The video is the
promotional artifact for that idea, used in the launch announcement. It is not
part of the compiler and nothing in the build depends on it.

## Rendering

```sh
cd video
npm install       # ~490 MB of node_modules — git-ignored
npm run render    # -> out/agentic-rain.mp4
```

`npm start` opens the Remotion studio for live editing.

## Files

| Path | Role |
| --- | --- |
| `src/DigitalRain.tsx` | The composition — per-column state is derived deterministically from a seed, so a re-render is byte-reproducible |
| `src/Root.tsx`, `src/index.ts` | Remotion entry points |
| `out/agentic-rain.mp4` | The rendered output. **No longer tracked** — see below |

## Getting the rendered video

It is a build output, not source, so it is git-ignored. Two ways to get it:

```sh
npm install && npm run render          # re-render it (deterministic — same bytes)
gh release download v0.2.0 -p '*.mp4'  # or fetch the release asset
```

## Two size notes

- **`node_modules/` is ~490 MB and ~8,500 files.** Git-ignored and regenerable
  with `npm install`. If you are measuring this repository's size on disk it
  dominates — and it is not in git.
- **The 37 MB mp4 was tracked until 2026-08-03** and is the single largest
  contributor to the ~159 MB `.git` directory. It is now untracked and ignored,
  so the repository stops *growing* — each re-render would otherwise have added
  another 37 MB blob forever.

  Untracking does **not** shrink `.git`: the blob remains in history, so a fresh
  clone still transfers it. Reclaiming that requires rewriting history, which
  moves every subsequent commit SHA, moves the `v0.2.0` tag, and forces every
  existing clone and fork to re-clone. That is a judgement call about who else
  has cloned the repo, so it is prepared rather than done:
  [`scripts/purge-video-from-history.sh`](../scripts/purge-video-from-history.sh)
  (dry-run by default). Attach the video to the release *before* running it.
