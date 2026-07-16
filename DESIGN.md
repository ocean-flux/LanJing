---
name: LanJing / 览境
description: Adaptive-frame local media workbench — semantic surfaces, lantern accent, quiet density
colors:
  canvas: '#f5f4f1'
  canvas-elevated: '#ffffff'
  ink: '#1f1e1c'
  ink-muted: '#706b64'
  ink-subtle: '#8a847c'
  surface-1: '#ffffff'
  surface-2: '#faf9f6'
  surface-3: '#f0efea'
  hairline: '#1f1e1c1a'
  lantern: '#c2683a'
  lantern-strong: '#9a4e2c'
  lantern-hover: '#8a4624'
  lantern-soft: '#c2683a24'
  lantern-tint: '#eee0d7'
  on-lantern: '#fdf8f1'
  reader-canvas: '#f4efe4'
  reader-ink: '#211e1a'
  media-void: '#e3e1dc'
  positive: '#557d59'
  warning: '#9f6d1e'
  danger: '#b83f4e'
  canvas-dark: '#141417'
  ink-dark: '#e8e6e1'
  surface-1-dark: '#1b1b1f'
  lantern-dark-ring: '#c2683a57'
typography:
  ui:
    fontFamily: "Outfit, Inter, ui-sans-serif, system-ui, 'Microsoft YaHei', sans-serif"
    fontSize: '0.875rem'
    fontWeight: 500
    lineHeight: 1.4
  body:
    fontFamily: "Outfit, Inter, ui-sans-serif, system-ui, 'Microsoft YaHei', sans-serif"
    fontSize: '1rem'
    fontWeight: 400
    lineHeight: 1.5
  reader:
    fontFamily: "Source Serif 4, Georgia, 'Songti SC', 'SimSun', serif"
    fontSize: '1.125rem'
    fontWeight: 400
    lineHeight: 1.75
  mono:
    fontFamily: 'JetBrains Mono, SFMono-Regular, Consolas, monospace'
    fontSize: '0.8125rem'
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: 'Outfit, Inter, ui-sans-serif, system-ui, sans-serif'
    fontSize: '0.75rem'
    fontWeight: 500
    lineHeight: 1.3
    letterSpacing: '0.01em'
rounded:
  sm: '6px'
  md: '10px'
  lg: '14px'
  xl: '20px'
  2xl: '28px'
  3xl: '34px'
spacing:
  xs: '4px'
  sm: '8px'
  md: '16px'
  lg: '24px'
  xl: '32px'
  section: '32px'
  card-gap: '16px'
  page-mobile: '16px'
  page-tablet: '24px'
  page-desktop: '32px'
  content-max: '1280px'
  reading-max: '680px'
components:
  button-primary:
    backgroundColor: '{colors.lantern-strong}'
    textColor: '{colors.on-lantern}'
    rounded: '{rounded.lg}'
    padding: '8px 10px'
    height: '32px'
  button-primary-hover:
    backgroundColor: '{colors.lantern-hover}'
    textColor: '{colors.on-lantern}'
  button-outline:
    backgroundColor: '{colors.canvas}'
    textColor: '{colors.ink}'
    rounded: '{rounded.lg}'
    padding: '8px 10px'
    height: '32px'
  button-ghost:
    backgroundColor: 'transparent'
    textColor: '{colors.ink}'
    rounded: '{rounded.lg}'
    padding: '8px 10px'
  surface-panel:
    backgroundColor: '{colors.surface-1}'
    textColor: '{colors.ink}'
    rounded: '{rounded.xl}'
    padding: '20px'
  nav-rail:
    backgroundColor: '{colors.canvas}'
    textColor: '{colors.ink}'
    width: '220px'
  input-default:
    backgroundColor: '{colors.canvas}'
    textColor: '{colors.ink}'
    rounded: '{rounded.lg}'
    padding: '8px 12px'
    height: '36px'
---

# Design System: LanJing / 览境

## 1. Overview

**Creative North Star: "The Adaptive Frame"**

LanJing is a product UI system for a local cross-media workbench. The Adaptive Frame is the constant: product orientation, navigation grammar, and semantic color roles stay stable. **Shell chrome is premium by quiet precision** — not by heavy atmosphere. **Immersion is modal**: novel/music/reader/player may change paper, void, and density **inside the content surface**; rail, titlebar, and bottom nav keep the same grammar and role names.

Default appearance pack: **纸灯精密 (Paper-Lantern Precision)** — warm-paper neutrals, copper lantern accent ≤~10% mass, compact controls, tonal elevation. Themes may later rebind hex via L2 packs; **roles** (`canvas`, `ink`, `lantern`, `reader-*`, `media-void`) are the contract, not any one hex romance.

The system rejects generic SaaS dashboard kits, content-farm feed noise, scraper-tool chrome, multi-player focus fights, and production UIs stuffed with fake fullness. Motion exists for state, not page theatre. Reduced motion and reduced transparency remain fully operable.

**Key Characteristics:**

- **Shell / mode split:** chrome = quiet precision; mode content = allowed immersion
- Semantic roles first; L2 appearance packs rebind values only
- Restrained lantern accent (primary actions, selection, focus)
- Outfit for chrome/UI; Source Serif 4 for long reading; JetBrains Mono for code/rules
- Tonal elevation (surface-1/2/3 + hairline); shadows only on controls/dialogs
- Adaptive shell metrics (rail, bottom nav, safe area, mini-player)
- Quiet density in chrome; reading may open measure/line-height without bloating nav

**Theme layers (implement against these names):**

| Layer | Responsibility |
| ----- | -------------- |
| L0 | `light` / `dark` / `system` on `documentElement` |
| L1 | CSS variables for semantic roles (this file’s palette) |
| L2 | Optional pack id rebinding L1 (default pack only in production until a multi-pack task) |
| L3 | Mode-scoped presentation tokens for main/reader/player — never rename L1 |
| Reader prefs | Independent of L0; paper/white/gray/dark/black etc. |

**Agent taste gate (product register):** Reject marketing-landing “premium” that fights this system: oversized hero padding in app chrome, gradient text, side-stripe cards, decorative glass on shell, Inter-as-brand, Lucide as pure decoration, identical card grids as default discovery, shell mood filters, neon media-void as global chrome. Prefer precision tools (Linear/Books calm) over Awwwards portfolio energy. Immersive hero treatments belong in **mode content**, not in the global frame.

## 2. Colors

Warm-neutral canvas with a single copper-lantern accent. Light and dark share roles; dark deepens canvas toward cool charcoal while ink flips to warm paper.

### Primary

- **Lantern** (`#c2683a`): brand warmth; soft fills (`lantern-soft`), tints, and hover glows — not full-bleed backgrounds.
- **Lantern Strong** (`#9a4e2c`): primary button / high-emphasis action fill (`primary`).
- **Lantern Hover** (`#8a4624`): pressed/hover deepen on lantern actions.
- **On Lantern** (`#fdf8f1`): text/icon on lantern fills.

### Neutral

- **Canvas** (`#f5f4f1` light / `#141417` dark): app shell ground.
- **Ink** (`#1f1e1c` / `#e8e6e1`): primary text.
- **Ink Muted / Subtle** (`#706b64`, `#8a847c` and dark peers): secondary labels; keep contrast honest — do not fade body copy into decoration.
- **Surface 1/2/3**: panel ladder (`#ffffff` → `#faf9f6` → `#f0efea` light; dark `#1b1b1f` → `#222327` → `#2b2c31`).
- **Hairline**: low-alpha ink borders (`rgb(31 30 28 / 0.1)` light).

### Reader & media

- **Reader Canvas / Ink** (`#f4efe4` / `#211e1a` light): immersive text reading, independent of shell theme when reader prefs demand it.
- **Media Void** (`#e3e1dc` light / `#23262d` dark): empty media placeholders without fake art.

### Semantic status

- **Positive** `#557d59`, **Warning** `#9f6d1e`, **Danger** `#b83f4e` (dark variants shift lighter for contrast on dark canvas).

### Named Rules

**The Role Stability Rule.** Themes may re-skin hex values (L2); they must not invent parallel meanings for `canvas` / `ink` / `lantern` / `reader-*`. Multi-pack configuration rebinds values, never forks a second shell language.

**The One Lantern Rule.** Lantern appears for primary action, selection, and focus affordance — roughly ≤10% of a screen’s color mass. If the page looks orange, lantern is overused.

**The Honest Empty Rule.** Prefer `media-void` + real copy over stock photos or fabricated shelves.

**The Quiet Shell Rule.** Rail, titlebar, bottom nav, and command chrome stay low-atmosphere. Do not apply full-bleed media tints, heavy blur stacks, or cinematic gradients to shell chrome to “feel immersive.”

**The Modal Immersion Rule.** Atmosphere (reader paper, album hero, void stage) applies inside `main` / reader / player surfaces. Leaving a mode restores shell-default L1 presentation without sticky mood on global nav.

## 3. Typography

**Display/UI Font:** Outfit (Inter / system-ui / Microsoft YaHei fallbacks)  
**Reader Font:** Source Serif 4 (Georgia / Songti SC / SimSun fallbacks)  
**Mono Font:** JetBrains Mono

**Character:** Geometric-humanist UI sans for chrome; literary serif only where long-form reading is the job. No display face in buttons, tabs, or data tables.

### Hierarchy

- **UI / Title** (Outfit 500–600, ~14–16px, LH ~1.4): shell labels, nav, dialog titles.
- **Body** (Outfit 400, 16px, LH 1.5): product prose; max ~65–75ch in reading-adjacent panels.
- **Reader body** (Source Serif 4 400/600, default ~18px, LH ~1.75): immersive text; content max ~680px (wide ~1120px).
- **Label** (Outfit 500, ~12px): meta, captions, badges — not all-caps tracking eyebrows by default.
- **Mono** (JetBrains Mono ~13px): rules, IDs, debug, technical witnesses.

### Named Rules

**The Serif-Only-When-Reading Rule.** Source Serif 4 is for reader surfaces and intentional literary moments — not for nav, buttons, or tables.

**The Density Type Rule.** Prefer the tighter product scale; do not inflate type to fill empty shell regions.

## 4. Elevation

Depth is **tonal first, shadow second**. Panels step through `surface-1` → `surface-2` → `surface-3` with 1px hairline borders. Shadows are structural responses (controls, dialogs), not card decoration.

### Shadow Vocabulary

- **Panel inset** (`0 1px 0 rgb(255 255 255 / 0.56) inset` light): quiet paper edge on panels.
- **Control** (`inset highlight + 0 4px 12px rgb(0 0 0 / 0.08)`): raised controls.
- **Dialog** (`0 16px 40px rgb(0 0 0 / 0.18)` light / stronger on dark): modal lift only.
- **Focus ring** (`0 0 0 2px rgb(154 78 44 / 0.24)`): lantern-tinted focus, always visible.

### Material

Optional soft material uses blur (`--material-blur: 16px`) when transparency is allowed. Coarse pointer, max-width 767px, `prefers-reduced-transparency`, and `data-material-transparency='low'` force solid surfaces — no information may depend on blur.

### Named Rules

**The Flat-By-Default Rule.** Resting content cards do not cast heavy drop shadows. If it looks like a 2014 elevated tile stack, reduce blur and darken hairline instead.

**The Transparency Is Optional Rule.** Reduced transparency must look intentional, not broken glass.

## 5. Components

Handfeel: **quiet and precise**, compact padding, fewer empty bands. shadcn-svelte (`nova`) primitives under `src/lib/components/ui/` compose with LanJing tokens.

### Buttons

- **Shape:** gently rounded (`rounded-lg` / ~14px family).
- **Primary:** `lantern-strong` fill, `on-lantern` text; hover `lantern-hover`; default height ~32px (`h-8`) for density.
- **Outline / Secondary / Ghost:** border or muted surface; never second saturated accent.
- **Focus:** ring token, not color-only change.
- **Disabled:** opacity ~50%, no pointer.

### Cards / Containers

- **Corner:** panel/shell often `radius-xl` (20px); controls `radius-lg`.
- **Background:** `surface-1` / `surface-panel`; border hairline.
- **Shadow:** panel inset only unless dialog/control.
- **Padding:** card pad ~20px; gaps `--card-gap` 16px — prefer content packing over airy marketing cards.
- **No nested card-in-card-in-card.**

### Inputs / Fields

- **Style:** hairline border, canvas/surface fill, `radius-lg`.
- **Focus:** lantern-tinted ring (`--ring` / focus-ring).
- **Error:** danger border + ring; message in danger ink.

### Navigation (Adaptive Frame)

- **Desktop expanded rail:** ~220px, canvas ground, hairline right edge — precision tool, not a mood panel.
- **Icon rail / tablet:** ~64–72px metrics via CSS vars.
- **Mobile:** bottom nav ~64px + `safe-area-inset-bottom`; never covered by mini-player reservation.
- **Titlebar:** ~40–44px; native window control modes from shell platform contract (`windows-overlay` / `macos-overlay` / …).
- **Active/hover:** soft lantern tint, not loud fill blocks or media-tint takeover.
- **Reader presentation:** shell chrome recedes (`motion-reader-recede`); no dual primary navs; immersion is the reader surface, not a restyled rail.

### Mini-player / ambient strip

- Height token ~56px; reserved vs visible states; label from real ambient session only — never a fake “now playing” when empty.

### Feedback

- Toasts via sonner; skeleton for loading; empty states teach the next real action (add source, import file).

## 6. Do's and Don'ts

### Do

- **Do** bind UI to semantic roles (`canvas`, `ink`, `lantern`, `reader-*`) so multi-theme can rebind later.
- **Do** keep one primary lantern action per region; use ghost/outline for the rest.
- **Do** preserve Adaptive Frame metrics (rail, bottom nav, safe area, mini-player) across breakpoints.
- **Do** use Source Serif only on reader/long-form surfaces.
- **Do** honor `prefers-reduced-motion` and reduced transparency with full task completion.
- **Do** pack shell chrome tightly; spend space on media content, not empty hero padding in app chrome.
- **Do** show honest empty and partial source states.

### Don't

- **Don't** ship generic SaaS dashboard chrome and metric-card kits as the product identity.
- **Don't** build content-farm or noisy infinite-feed surfaces as the default discovery language.
- **Don't** expose scraper/pipeline guts as primary UI (“scraper tool as product shell”).
- **Don't** run multiple competing players or a second foreground activity fighting focus.
- **Don't** fill production with fake data, prototype variants, or demo fullness.
- **Don't** use side-stripe accent borders, gradient text, or decorative glassmorphism as defaults.
- **Don't** put display/serif type in buttons, tabs, or dense data.
- **Don't** invent a second color system beside the semantic roles for “just this page.”
- **Don't** inflate whitespace to look “premium” while hiding information density users need.
- **Don't** put heavy atmosphere (cinematic gradients, full media tint, decorative glass stacks) on shell chrome.
- **Don't** make L0 light/dark a different information architecture than each other.
- **Don't** ship multiple L2 packs in production until a dedicated multi-pack task; keep the AppearancePack seam typed and default-only.
