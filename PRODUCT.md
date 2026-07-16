# Product

## Register

product

## Platform

adaptive

## Users

Primary users are local-first, heavy media consumers — people who read, listen, and browse across many sources on desktop or mobile, often for long sessions. They care about continuity (where they are, what is playing, what they own) more than about configuring a content farm.

Context of use: a personal workstation or phone; no cloud account; sources and progress live on the device. Job to be done: discover, understand, collect, read, and play media from heterogeneous sources without losing orientation when switching spaces or devices form factors.

Success looks like one continuous workbench: stable product orientation, a single shared library truth, ambient audio that can ride along silent activities, and honest empty states when there is no real data.

## Product Purpose

LanJing (览境) is a rule-driven local media workbench. Rules only emit standard media models; LanJing owns discovery, presentation, library, reading, and playback with high-quality product templates.

It exists so multi-source content can be consumed with the calm of a crafted app shell rather than the noise of a scraper UI. Success is measured by cross-media continuity, trustworthy emptiness, and immersive consumption that still respects platform navigation and accessibility.

## Positioning

A rule-driven local cross-media workbench: rules supply standard data; LanJing supplies elegant discovery, understanding, collection, reading, and playback — with a stable frame and immersive modes per media.

## Brand Personality

Calm, craft-minded, continuous. Voice is precise and quiet, never hype. The interface should feel like a trustworthy immersion: a variable picture frame that holds media without shouting, with session continuity across spaces.

## Anti-references

- Generic SaaS dashboard chrome and metric-card kits
- Content-farm or noisy infinite-feed surfaces
- “Scraper tool as product shell” layouts that expose pipeline guts as UI
- Multiple competing players or second foreground activities fighting focus
- Demo fullness that fakes data, prototype variants, or marketing placeholders in production

## Design Principles

1. **Frame holds media, media does not fight the frame** — global orientation stays legible while media spaces may change density and atmosphere.
2. **One foreground, optional ambient** — a single activity owns attention; ambient audio may continue only when it does not seize focus.
3. **One library truth** — media spaces project the shared library; they never own a second copy of favorites, history, or progress.
4. **Honest emptiness** — no fabricated recommendations or filled shells; empty states name the real next step.
5. **Local-first continuity** — platform, orientation, and route changes must not casually reset theme preference, reading place, or ambient session.

## Accessibility & Inclusion

Target WCAG 2.2 AA for interactive product surfaces: text contrast ≥ 4.5:1 (large text ≥ 3:1), visible focus, full keyboard paths on desktop, touch-complete paths on mobile.

Reduced motion and reduced transparency are first-class product capabilities: core tasks remain complete with equivalent information hierarchy when those preferences are on. Reader and media surfaces must not rely on motion or translucency alone to convey state.

## Design System Routing (for agents)

Visual source of truth: root `PRODUCT.md` + `DESIGN.md` (+ `.impeccable/design.json`). Trellis frontend tasks must load these before shaping UI.

**Register dials (product, not marketing):**

| Dial | Value | Meaning |
| ---- | ----- | ------- |
| Variance | 4–5 | Consistent shell grammar; media spaces vary atmosphere, not IA |
| Motion | 3–4 | State / feedback only; no page theatre |
| Density | 6–7 | Quiet density; less empty chrome air; content packs tighter than landing pages |

**Skill routing when UI changes:**

1. Always: `PRODUCT.md`, `DESIGN.md`, `.trellis/spec/frontend/*`, domain context if media.
2. Primary craft: project `impeccable` skill with **product** register (`reference/product.md`); platform `adaptive` → also respect iOS/Android notes when chrome is native-adjacent.
3. Implementation helpers: `tailwind`, `svelte-core-bestpractices`, `svelte-code-writer`.
4. Do **not** default to landing/agency skills (`design-taste-frontend`, `high-end-visual-design`, `gpt-taste`, imagegen web/mobile landing packs) for app shell, library, reader, or player work — those skills optimize marketing variance and macro-whitespace that fight Adaptive Frame density.
5. Use marketing/image skills only when the task is explicitly brand/marketing surface.
