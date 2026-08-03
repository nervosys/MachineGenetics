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
| `out/agentic-rain.mp4` | The rendered output, **committed** (38 MB) |

## Two size notes

- **`node_modules/` is ~490 MB and ~8,500 files.** It is git-ignored (`node_modules`
  in the root `.gitignore`) and regenerable with `npm install`. If you are
  measuring this repository's size on disk, it dominates — and it is not in git.
- **`out/agentic-rain.mp4` is 38 MB and *is* committed**, which is the single
  largest contributor to the ~158 MB `.git` directory. That is a deliberate
  convenience so the artifact is available without a render toolchain. If clone
  time ever matters, move it to a GitHub release asset or Git LFS — tracked as
  open item 5 in [`ROADMAP.md`](../ROADMAP.md).
