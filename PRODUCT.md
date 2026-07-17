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

Calm, craft-minded, continuous. Voice is precise and quiet, never hype.

**Split feel (critical):** the **shell is premium by quiet precision** — fine hairlines, compact controls, restrained material, no atmospheric filter on chrome. **Immersion lives inside media modes** (reader paper, cover density, player stage), not by dressing the global frame as a mood board. The Adaptive Frame stays legible; content surfaces may deepen atmosphere.

## Anti-references

- Generic SaaS dashboard chrome and metric-card kits
- Content-farm or noisy infinite-feed surfaces
- “Scraper tool as product shell” layouts that expose pipeline guts as UI
- Multiple competing players or second foreground activities fighting focus
- Demo fullness that fakes data, prototype variants, or marketing placeholders in production

## Design Principles

1. **Frame holds media, media does not fight the frame** — global orientation stays legible while media spaces may change density and atmosphere **on the content surface only**.
2. **Shell premium ≠ shell atmosphere** — advanced feel comes from precision and craft in chrome, not from heavy mood, neon, or decorative glass on spine rail/titlebar/bottom nav.
3. **Immersion is modal** — novel/music/reader/player may deepen paper, void, and density inside main; they must not rewrite primary nav grammar or semantic role names.
4. **One foreground, optional ambient** — a single activity owns attention; ambient audio may continue only when it does not seize focus.
5. **One library truth** — media spaces project the shared library; they never own a second copy of favorites, history, or progress.
6. **Honest emptiness** — no fabricated recommendations or filled shells; empty states name the real next step.
7. **Local-first continuity** — platform, orientation, and route changes must not casually reset theme preference, reading place, or ambient session.

## Theme & Appearance Model

| Layer                  | What                                                              | Production now                                                                                                                                                                                                                                                     |
| ---------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **L0 Appearance mode** | `light` \| `dark` \| `system`                                     | Required; user explicit choice beats system                                                                                                                                                                                                                        |
| **L1 Semantic roles**  | `canvas` / `ink` / `lantern` / `reader-*` / `media-void` / status | Required contract; names stable forever                                                                                                                                                                                                                            |
| **L2 Theme (faces)**   | Rebind L1 hex/material only; one IA                               | Product copy: **主题**. Built-in **墨砚精密** / **冷银朱**. User picks **light-face theme** and **dark-face theme** separately in settings; `system` mode resolves OS face then applies that track. No auto-invert. Legacy single pack id migrates to both tracks. |
| **L3 Mode atmosphere** | Content-surface density/paper/void inside a media mode            | Allowed as presentation, not a second shell language                                                                                                                                                                                                               |
| **Reader prefs**       | Independent colorScheme/font/size/…                               | Independent of L0; survives app theme changes                                                                                                                                                                                                                      |

**Default theme character:** near-neutral canvas + teal-ink lantern accent at ≤~10% mass; quiet precision chrome; reader paper slightly warmer than shell; not cream+copper dual-warm, not media-neon void as global shell.

**Legacy:** `paper-lantern-precision` and single `appearancePackId` storage map to inkstone on both tracks; not official UI labels.

**Shell chrome (current):** desktop **48px icon spine** (four realms + settings entry, collapsible; left-edge hover restores); **minimal titlebar** (context + window controls only — no search/settings/theme cycle); mobile context toolbar + four-realm bottom nav; reader hides ordinary chrome. List/home content uses full main width (not magazine-narrow columns).

**Out of scope for now:** user-authored theme marketplace, cloud sync, per-source skins, cover-derived shell tint.

## Accessibility & Inclusion

Target WCAG 2.2 AA for interactive product surfaces: text contrast ≥ 4.5:1 (large text ≥ 3:1), visible focus, full keyboard paths on desktop, touch-complete paths on mobile.

Reduced motion and reduced transparency are first-class product capabilities: core tasks remain complete with equivalent information hierarchy when those preferences are on. Reader and media surfaces must not rely on motion or translucency alone to convey state.

## Design System Routing (for agents)

Visual source of truth: root `PRODUCT.md` + `DESIGN.md` (+ `.impeccable/design.json`). Trellis frontend tasks must load these before shaping UI.

**Register dials (product, not marketing):**

| Dial     | Shell chrome | Content / mode surfaces                                      |
| -------- | ------------ | ------------------------------------------------------------ |
| Variance | 3–4          | 5–6 atmosphere on content only; IA still shared              |
| Motion   | 3            | 3–4 state feedback; immersive modes may use reader page cues |
| Density  | 6–7          | 5–7; reading may open line-height, chrome stays compact      |

**Skill routing when UI changes:**

1. Always: `PRODUCT.md`, `DESIGN.md`, `.trellis/spec/frontend/*`, domain context if media.
2. Primary craft: project `impeccable` skill with **product** register (`reference/product.md`); platform `adaptive` → also respect iOS/Android notes when chrome is native-adjacent.
3. Implementation helpers: `tailwind`, `svelte-core-bestpractices`, `svelte-code-writer`.
4. Do **not** default to landing/agency skills (`design-taste-frontend`, `high-end-visual-design`, `gpt-taste`, imagegen web/mobile landing packs) for app shell, library, reader, or player work — those skills optimize marketing variance and macro-whitespace that fight Adaptive Frame density.
5. Use marketing/image skills only when the task is explicitly brand/marketing surface.
