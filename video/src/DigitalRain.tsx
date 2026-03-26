import React from "react";
import { useCurrentFrame, useVideoConfig, random } from "remotion";

// ── MechGen Agent-mode sigils only ──────────────────────────
// The compressed single/multi-char tokens that agents emit.
const AGENT_TOKENS = [
  "f", "+", "v", "m", "c", "S", "E", "T", "I", "M", "u", "Y", "Z",
  "af", ".w", "?", ":", "?=", "@", "@w", "@@", "!", ">>", "ret", "~>",
  "&m", "->", "=>", ";", "{", "}", "(", ")", "<", ">", "|", "#",
];

// ── Colour palette ──────────────────────────────────────────
const GREEN_BRIGHT = "#00ff41";
const GREEN_MID = "#00cc33";
const GREEN_DIM = "#005500";
const BG = "#0a0a0a";

// ── Per-column state (deterministic from seed) ──────────────
interface Column {
  x: number;        // pixel offset
  speed: number;    // cells per frame (fractional)
  length: number;   // trail length in cells
  offset: number;   // starting row offset (stagger)
  tokens: string[]; // pre-rolled token sequence
}

function buildColumns(
  width: number,
  cellW: number,
  cellH: number,
  rows: number,
  seed: string,
): Column[] {
  const cols = Math.floor(width / cellW);
  const columns: Column[] = [];

  for (let i = 0; i < cols; i++) {
    const s = `${seed}-col-${i}`;
    const speed = 0.3 + random(s + "-spd") * 0.7;          // 0.3 – 1.0
    const length = Math.floor(6 + random(s + "-len") * 20); // 6 – 25
    const offset = Math.floor(random(s + "-off") * rows * 3);

    // Pre-generate enough tokens for a full cycle
    const cycle = rows + length + 40;
    const tokens: string[] = [];
    for (let t = 0; t < cycle; t++) {
      const idx = Math.floor(random(`${s}-t-${t}`) * AGENT_TOKENS.length);
      tokens.push(AGENT_TOKENS[idx]);
    }

    columns.push({ x: i * cellW, speed, length, offset, tokens });
  }
  return columns;
}

// ── Single column renderer ──────────────────────────────────
const RainColumn: React.FC<{
  col: Column;
  frame: number;
  cellW: number;
  cellH: number;
  rows: number;
  height: number;
}> = ({ col, frame, cellW, cellH, rows, height }) => {
  // Current head position (wraps around)
  const totalRows = rows + col.length + 10;
  const headRow = (col.offset + frame * col.speed) % totalRows;

  const cells: React.ReactNode[] = [];

  for (let r = 0; r < rows; r++) {
    const dist = headRow - r; // distance behind head
    if (dist < 0 || dist > col.length) continue;

    // Brightness fades along the trail
    const ratio = 1 - dist / col.length;
    let color: string;
    let opacity: number;
    let fontWeight: string | number = 400;

    if (dist < 1) {
      // Head cell — white-hot
      color = "#ffffff";
      opacity = 1;
      fontWeight = 700;
    } else if (ratio > 0.6) {
      color = GREEN_BRIGHT;
      opacity = 0.7 + ratio * 0.3;
      fontWeight = 600;
    } else if (ratio > 0.25) {
      color = GREEN_MID;
      opacity = 0.4 + ratio * 0.4;
    } else {
      color = GREEN_DIM;
      opacity = ratio * 1.6;
    }

    // Occasional flicker — swap token for 1 frame
    const tokenIdx = (r + Math.floor(frame * col.speed)) % col.tokens.length;
    const flicker = random(`flk-${col.x}-${r}-${frame}`) > 0.92;
    const token = flicker
      ? AGENT_TOKENS[Math.floor(random(`flk2-${col.x}-${r}-${frame}`) * AGENT_TOKENS.length)]
      : col.tokens[tokenIdx];

    cells.push(
      <span
        key={r}
        style={{
          position: "absolute",
          left: col.x,
          top: r * cellH,
          width: cellW,
          height: cellH,
          color,
          opacity,
          fontWeight,
          fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
          fontSize: cellH * 0.8,
          lineHeight: `${cellH}px`,
          textAlign: "center",
          textShadow: dist < 2
            ? `0 0 8px ${GREEN_BRIGHT}, 0 0 20px ${GREEN_MID}`
            : `0 0 4px ${GREEN_DIM}`,
          willChange: "opacity",
        }}
      >
        {token}
      </span>,
    );
  }

  return <>{cells}</>;
};

// ── Main composition ────────────────────────────────────────
export const DigitalRain: React.FC = () => {
  const frame = useCurrentFrame();
  const { width, height } = useVideoConfig();

  const cellW = 22;
  const cellH = 26;
  const rows = Math.ceil(height / cellH) + 2;

  const columns = React.useMemo(
    () => buildColumns(width, cellW, cellH, rows, "mechgen"),
    [width, cellW, cellH, rows],
  );

  return (
    <div
      style={{
        width,
        height,
        backgroundColor: BG,
        overflow: "hidden",
        position: "relative",
      }}
    >
      {/* Scanline overlay */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          background:
            "repeating-linear-gradient(0deg, rgba(0,0,0,0.15) 0px, rgba(0,0,0,0.15) 1px, transparent 1px, transparent 3px)",
          pointerEvents: "none",
          zIndex: 10,
        }}
      />

      {/* Vignette */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          background:
            "radial-gradient(ellipse at center, transparent 50%, rgba(0,0,0,0.7) 100%)",
          pointerEvents: "none",
          zIndex: 11,
        }}
      />

      {/* Rain columns */}
      {columns.map((col, i) => (
        <RainColumn
          key={i}
          col={col}
          frame={frame}
          cellW={cellW}
          cellH={cellH}
          rows={rows}
          height={height}
        />
      ))}

      {/* Title watermark */}
      <div
        style={{
          position: "absolute",
          bottom: 40,
          left: 0,
          width: "100%",
          textAlign: "center",
          zIndex: 20,
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: 28,
          fontWeight: 700,
          color: GREEN_BRIGHT,
          opacity: 0.25 + 0.1 * Math.sin(frame * 0.05),
          textShadow: `0 0 20px ${GREEN_BRIGHT}, 0 0 40px ${GREEN_MID}`,
          letterSpacing: 8,
          textTransform: "uppercase",
        }}
      >
        MECHGEN · AGENTIC RAIN
      </div>
    </div>
  );
};
