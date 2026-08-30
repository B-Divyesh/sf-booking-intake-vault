# Private Intake — visual thesis

## Direction

Private Intake uses an **art-deco transit poster** language: precise routes, punched tickets, brass wayfinding and midnight station boards. That world fits a field-service intake tool because information should travel deliberately to the right destination. Decoration always explains the privacy model: a gold route carries permitted job facts to a worker; a coral route terminates inside the manager's vault.

This is intentionally a single dark treatment. The midnight field makes the separation between public intake, private vault and worker brief feel spatial, while warm paper surfaces keep long forms comfortable and legible.

## Palette

- `midnight #101C2C` — page background and vault context.
- `midnight-raised #17283A` — navigation and supporting surfaces.
- `paper #F4EBD8` — primary work surface.
- `paper-deep #E6D8BD` — inset controls and dividers.
- `ink #142233` — text on paper.
- `mist #B8C7CE` — secondary text on midnight (7.7:1).
- `brass #E5B84B` — primary action, route, focus (8.8:1 against midnight).
- `brass-ink #1C2430` — text on brass.
- `coral #ED765F` — private/admin marker and warnings.
- `signal #78C6A3` — success and worker-safe marker.
- `danger #C94F52` — destructive state, always paired with words/icons.

## Type

No runtime font downloads. Display headings use `Georgia, Cambria, Times New Roman, serif` in compact, high-contrast capitals, evoking engraved station titling without importing a novelty face. Interface copy uses `Inter, ui-sans-serif, system-ui, sans-serif` for fast, familiar scanning. The scale is 16, 18, 22, 30, 44 and 64 px; body line-height is 1.55 and text measures never exceed 72 characters.

## Layout and spacing

An 8 px base rhythm with 4 px detail spacing. Work surfaces use clipped deco corners rather than generic rounded cards. Fine double rules are reserved for major route boundaries. Desktop uses a 12-column station-board grid; at 390 px every task becomes a single linear route, secondary decoration drops away, tables become labeled stacked records, and actions remain at least 44 px.

## Interaction grammar

- Brass lozenges are primary actions; paper-outline controls are secondary.
- A vertical route line connects stages and status events.
- Visibility is never color-only: each intake field carries `Worker sees` or `Manager only` text and a distinct eye/lock symbol.
- Worker previews are intentionally isolated inside a navy ticket frame so managers can verify the exact disclosure boundary.
- Loading uses short skeleton bars; empty states explain the next operational step; offline and errors offer an explicit retry.

## Motion

UI changes use 180–240 ms opacity and transform transitions. Route markers move only once when a stage changes; no animation loops. With `prefers-reduced-motion: reduce`, transitions and transforms become instant while borders, labels and spatial hierarchy preserve every state.

## Original asset plan and provenance

The hero is a generated landscape poster used as atmosphere beside the booking proposition. It depicts an abstract night station with one public route splitting into a lit worker platform and a sealed manager vault, with no people, brands, text or simulated UI. Decorative route marks and icons elsewhere are hand-authored CSS/SVG primitives.

Prompt sheet:

> Use case: stylized-concept. Asset type: responsive website hero illustration. Primary request: an elegant 1930s art-deco transit poster as a visual metaphor for privacy-aware field-service booking. Scene: a midnight blue geometric station concourse where one intake route cleanly separates into two destinations, a warm lit worker platform with only a tool case and simple work order, and a sealed coral-and-brass manager vault holding private papers. Style: screen-printed vintage travel poster, crisp geometric architecture, subtle paper grain, restrained flat shapes, premium editorial finish. Composition: landscape 3:2, strong diagonal route, clear focal separation, no central text area required. Palette: midnight navy, warm ivory, aged brass, coral red, muted sea green. Lighting: theatrical pools of warm light, calm and trustworthy. Constraints: abstract objects only, no people, no readable text, no letters, no numbers, no logos, no watermark, no brands, no gradients, no padlock cliché, no modern device mockups.

- Generator: factory Azure image deployment via `/opt/fleet/lib/gen-image.sh`.
- Date: 2026-08-28.
- License/provenance: original AI-generated artwork commissioned for this product; no source image, brand or copyrighted character used. Prompt sidecar retained under `assets/src/`.
- The 1200×630 social card is a center crop of the original 1200 px hero.
- The 180 px touch icon is a hand-authored geometric mark using the documented midnight and brass palette.
