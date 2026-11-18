---
name: Panelport
description: Local web panels as native windows, in a night-blue world where the selected panel owns the stage.
colors:
  night-deep: "#070d16"
  night-mid: "#0f2236"
  night-glow: "#1d4363"
  bar-end: "#132b44"
  white: "#ffffff"
  ink-on-white: "#0b1622"
  state-up: "#5ee39a"
  state-warn: "#ffc44d"
  state-down: "#ff6b6b"
  state-down-text: "#ff8a8a"
  close-red: "#e5484d"
  log-black: "#03070d"
  log-ink: "#c9d6e2"
  mist-light: "#f6f9fc"
  mist-mid: "#e4edf6"
  mist-deep: "#ccdaea"
  state-up-text-light: "#127a45"
  state-warn-text-light: "#8a5800"
  state-down-text-light: "#b3242c"
typography:
  display:
    fontFamily: "Antonio, Arial Narrow, sans-serif"
    fontSize: "128px"
    fontWeight: 200
    lineHeight: 0.86
    letterSpacing: "0.02em"
  headline:
    fontFamily: "Antonio, Arial Narrow, sans-serif"
    fontSize: "44px"
    fontWeight: 300
    lineHeight: 1
    letterSpacing: "0.04em"
  title:
    fontFamily: "Antonio, Arial Narrow, sans-serif"
    fontSize: "30px"
    fontWeight: 300
    lineHeight: 1
    letterSpacing: "0.06em"
  body:
    fontFamily: "Manrope, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Manrope, Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    letterSpacing: "0.16em"
  state:
    fontFamily: "Manrope, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 600
  mono:
    fontFamily: "JetBrains Mono, Consolas, monospace"
    fontSize: "13px"
    fontWeight: 500
rounded:
  input: "10px"
  row: "12px"
  card: "14px"
  stage: "18px"
  pill: "99px"
spacing:
  bar: "44px"
  caption-button: "46px"
  page-x: "40px"
  page-y: "34px"
  column-gap: "44px"
components:
  button-pill:
    backgroundColor: "rgba(255,255,255,0.06)"
    textColor: "{colors.white}"
    rounded: "{rounded.pill}"
    padding: "10px 20px"
  button-solid:
    backgroundColor: "{colors.white}"
    textColor: "{colors.ink-on-white}"
    rounded: "{rounded.pill}"
    padding: "10px 20px"
  button-fix:
    backgroundColor: "{colors.state-down}"
    textColor: "#1a0505"
    rounded: "{rounded.pill}"
    padding: "10px 20px"
  ring-action:
    backgroundColor: "rgba(255,255,255,0.06)"
    rounded: "{rounded.pill}"
    size: "54px"
  ring-action-primary:
    backgroundColor: "{colors.white}"
    rounded: "{rounded.pill}"
    size: "54px"
  list-item-selected:
    backgroundColor: "rgba(255,255,255,0.10)"
    rounded: "{rounded.row}"
    padding: "11px 14px"
  input:
    backgroundColor: "rgba(255,255,255,0.06)"
    textColor: "{colors.white}"
    rounded: "{rounded.input}"
    padding: "9px 12px"
  title-bar:
    height: "{spacing.bar}"
    textColor: "{colors.white}"
---

# Design System: Panelport

## Overview

**Creative North Star: "The Night Feed"**

Panelport is a quiet night-blue room where one panel at a time owns the stage, the way one item
fills a vertical video feed. The selected panel's own picture sits under a dark lower scrim and
its name is set huge in thin condensed capitals across the bottom edge; everything else (the
list of panels, the ringed actions, the title bar) stays white, thin and out of the way. The
category's grey grid of identical status cards is what this world refuses.

Density is low on the stage and moderate in lists. Type carries the identity: tall condensed
display caps for names, a calm rounded sans for interface text, a mono only for ports and
milliseconds. Colour does one job: green, amber and red say a panel's state, always with the
state in words beside the dot.

**Key Characteristics:**
- A radial night-blue ground with a darker floor; white is the only interface colour.
- Panel names in Antonio caps: huge on the stage and down page, 30 px in lists.
- State shown as a ringed dot plus the state word in the state's colour, never colour alone.
- Hairline white pills and ringed glyph buttons; the primary action is the filled white ring.
- Every window wears the same 44 px title bar with thin white caption glyphs.

## Colors

A monochrome night palette (blues stepping toward black) with white interface ink and three
state colours used only for state.

### Primary
- **Interface White** (#ffffff): all text, hairlines (at 13–34% opacity), pill borders, the filled
  primary ring and the solid button. In light mode its role passes to **Ink on White**
  (#0b1622).

### Neutral
- **Night Deep** (#070d16): the outer ground and the floor gradient.
- **Night Mid** (#0f2236): the middle of the ground and the title-bar gradient.
- **Night Glow** (#1d4363): the radial glow at the ground's upper right.
- **Bar End** (#132b44): the right end of the title-bar gradient, under the caption buttons.
- **Log Black** (#03070d) with **Log Ink** (#c9d6e2): container log boxes.
- **Mist Light / Mist Mid / Mist Deep** (#f6f9fc, #e4edf6, #ccdaea): the light-mode ground.

### Tertiary (state only)
- **Running Green** (#5ee39a): dot and word for a panel that answers; also the "on" toggle.
- **Attention Amber** (#ffc44d): server error, and "not a web page / no access".
- **Down Red** (#ff6b6b; word #ff8a8a): not reachable; the fill of the one fix button
  ("Start containers", "Start Docker Desktop").
- **Close Red** (#e5484d): the close caption button's hover only.
- Light mode keeps the dot hues and darkens the state words (#127a45, #8a5800, #b3242c).

### Named Rules
**The State-Only Colour Rule.** Green, amber and red appear only where they mean a panel's
state, the fix for it, or an on switch. No decorative colour anywhere else.

**The Words-With-Dots Rule.** A state dot is always followed by the state in words.

## Typography

**Display Font:** Antonio (with Arial Narrow), weights 200–300
**Body Font:** Manrope (with Segoe UI, system-ui)
**Label/Mono Font:** JetBrains Mono (with Consolas), weight 500

**Character:** tall, thin, condensed capitals carry every panel's name like a poster; Manrope
keeps controls friendly and readable; the mono appears only where a value is technical.

### Hierarchy
- **Display** (200, 128 px on the stage scaling down to 56 px, 112 px on the down page, line
  height .86, caps): the selected or affected panel's name.
- **Headline** (300, 44 px, caps): view headings (Paneller, "3 panel bulundu").
- **Title** (300, 30 px list / 26 px settings, caps, .06em): panel names in lists.
- **Body** (400, 14 px, 1.45; lead text 16–18 px): interface copy, measure about 34–64ch.
- **Label** (600, 12 px, .16em caps, 56% white): list and section labels that head their block.
- **State** (600, 14 px in lists, 17 px on the stage and down page): the state word.
- **Mono** (500, 12–13 px, tabular): ports, addresses, milliseconds, logs.

### Named Rules
**The English Caps Rule.** Uppercased panel names render with `lang="en"` unless they contain
Turkish letters, so names never pick up a dotted capital İ.

## Layout

The launcher is a three-column stage: a panel list (clamp 240–340 px), the stage (fluid), and a
96 px rail of ringed actions, with 24–44 px gaps and 20–40 px page padding that tighten with the
window. Below 1100 px the list's state moves under each name and the rail narrows to 76 px; the
launcher's minimum window is 900×600. Find panels is a list beside a 280–360 px summary card;
Settings is a 160–220 px section nav beside a single scrolling column capped at 860 px. The down
page is a fluid column beside a 280–420 px detail column that stacks below 820 px.

## Elevation & Depth

Depth comes from the atmospheric ground and one lifted object, the stage. Everything else sits
flat on the ground and separates with translucent white fills and hairlines.

### Shadow Vocabulary
- **Stage** (`box-shadow: 0 30px 80px rgba(0,0,0,.55)`; light `0 30px 70px rgba(37,66,99,.28)`):
  the stage only.
- **Toast** (`0 12px 30px rgba(0,0,0,.35)`): transient messages.

### Named Rules
**The One Lifted Object Rule.** Only the stage casts a shadow; lists, cards and rows stay flat.

## Shapes

Soft rectangles with generous corners (10 px inputs, 12 px rows, 14 px cards and steps, 18 px
stage) and full pills for every button. The caption buttons are square cells (46×44) with no
radius. Hairlines are 1 px white at low opacity; no thick borders.

## Components

### Buttons
- **Shape:** full pill (99px), 1 px white hairline at 34%.
- **Pill:** 6% white fill, white text, 600 13.5 px; hover lifts the fill to 10%.
- **Solid:** white fill, ink text: the main action of a card or empty state.
- **Fix:** Down Red fill with near-black text: only "Start containers" / "Start Docker Desktop".
- **Focus:** 2 px white outline, 3 px offset.

### Ring actions (signature)
54 px circles (46/44 px when narrow) with a 22 px line icon and a 12 px label under them; the
primary ring is filled white with an ink icon. Used in the launcher rail: Aç, Yenile, Günlük
(Docker panels only), Düzenle.

### Lists
Rows of condensed caps names with a state on the right (under the name when narrow); the selected
row has a 10% white fill and an inset hairline. Settings rows add a dotted grip for drag reorder
(Alt+Up / Alt+Down from the keyboard).

### Inputs / Fields
10 px radius, 6% white fill, 34% hairline; focus is a 2 px white outline. Toggles are 46×26 pills
that turn Running Green when on.

### Title bar
44 px gradient bar (Night Mid to Bar End) with the PANELPORT wordmark, a hairline separator,
tabs or the panel's dot, name and address, then pills and three 46 px caption cells with 1.15 px
white glyphs; close turns Close Red on hover. A native hit area over the caption cells gives them
the system's hit-test codes.

### Stage (signature)
The selected panel's stored preview, top-left anchored, under a lower scrim; the display name
over the bottom edge; a meta row of address (mono), the large state, response time (mono) and
container.

## Do's and Don'ts

### Do:
- **Do** keep white the only interface colour and reserve green, amber and red for state.
- **Do** set panel names in Antonio caps with `lang="en"` for non-Turkish names.
- **Do** pair every state dot with the state word, at 14 px or larger.
- **Do** keep the stage the only shadowed object.
- **Do** load fonts from the app's own files; nothing from the network.

### Don't:
- **Don't** add labels above headings; status and scan lines go below their heading.
- **Don't** introduce a second accent colour or coloured side borders.
- **Don't** show a live panel inside the launcher; only its stored preview.
- **Don't** use looping horizontal loaders; a breathing dot marks ongoing checks.
