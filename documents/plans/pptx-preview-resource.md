# Preview Phase for pptx-official Skill

Status: proposed

## Problem and outcome

The current pptx-official pipeline asks users to confirm theme and outline via
text-based `ask_user` before building. Users cannot *see* what the themes
look like or how slides will be arranged until the final .pptx is generated.
This wastes round-trips: a wrong theme choice means regenerating the entire
deck.

Adding a preview phase between Phase 2 (outline) and Phase 4 (build) lets
users visually evaluate themes and slide layouts *before* any .pptx file is
written, reducing rework and improving confidence.

## Scope

This document defines the `preview.md` resource to be added to the
pptx-official skill bundle. It does not modify existing resources.

## Prerequisites

The agent must have access to the `show_widget` tool (via
`visualize_read_me` + `show_widget`). When `show_widget` is unavailable,
the pipeline falls back to the current text-based flow with no preview.

## When to trigger preview

| Condition | Action |
|-----------|--------|
| User says "preview trước", "show me", "xem trước", or similar | Trigger preview |
| Deck > 10 slides AND material is complex/contested | Trigger preview |
| Deck goes to board, customer, regulator, public audience | Trigger preview |
| User says "just build it", "tạo nhanh", or delegates fully | Skip preview |
| Deck is ≤ 5 slides and straightforward | Skip preview |
| Non-interactive run (scheduled task) | Skip preview |

## Preview types

### 1. Theme picker

Renders 2-3 recommended themes as visual cards, each showing:
- Theme name and description
- Color swatches (bg, surface, accent, positive, negative)
- Typography sample (title + body font at approximate sizes)
- Suitability note ("best for executive reviews", "internal analytics")
- Contrast WCAG badge

**Interaction**: Each card has a button. Clicking sends a prompt to chat:
`"Theme: Boardroom"` or `"Theme: Ledger"`. The agent reads this as the
user's selection and continues.

**Widget dimensions**: width=900, height=700 (auto-grows to fit cards).

### 2. Slide preview grid

Renders all slides as compact cards in a scrollable grid:
- Slide number in top-left corner
- Action title (the thesis sentence, not a topic label)
- Layout label (title-only, content, section-divider, comparison, etc.)
- Body content preview (first 2-3 lines or placeholder shape indicators)
- Visual form label (chart, table, stat callout, image placeholder, etc.)
- Theme colors applied to title bar and background

**Interaction**: Read-only. A summary line at bottom shows total slide count
and the ghost deck test result ("Action titles alone tell the complete
argument: YES/NO").

**Widget dimensions**: width=1100, height=800 (auto-grows for >8 slides).

### 3. Style comparison

Renders 2-3 approach cards side by side for the same deck:
- Argument-first: light ground, one accent, no decorative imagery
- Visual-first: hero images, bold typography, dark accents
- Data-first: charts and tables dominate, minimal text

Each card shows a miniature slide sample (cover + one content slide) with
the approach applied. Does not require the full outline.

**Interaction**: Button sends `"Style: argument-first"` to chat.

**Widget dimensions**: width=1100, height=600.

## HTML template patterns

All templates follow the streaming-first architecture:
1. `<style>` block with CSS variables first
2. Content structure
3. `<script>` block last (for interactivity)

### Theme card template

```html
<style>
  :root {
    --bg: #0F1B2A;
    --surface: #17263A;
    --title: #F5F7FA;
    --body: #D5DEE9;
    --muted: #9FB0C4;
    --accent: #4F9CF9;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--body); }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 16px; padding: 16px; }
  .card {
    background: var(--surface);
    border-radius: 12px;
    padding: 20px;
    border: 2px solid transparent;
    transition: border-color 0.2s;
  }
  .card:hover { border-color: var(--accent); }
  .swatches { display: flex; gap: 6px; margin: 12px 0; }
  .swatch { width: 28px; height: 28px; border-radius: 6px; border: 1px solid rgba(255,255,255,0.1); }
  .title-sample { font-size: 22px; font-weight: 700; color: var(--title); margin: 8px 0 4px; }
  .body-sample { font-size: 14px; color: var(--body); line-height: 1.5; }
  .label { font-size: 11px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 4px; }
  .badge { display: inline-block; font-size: 10px; padding: 2px 8px; border-radius: 4px; background: rgba(255,255,255,0.08); color: var(--muted); margin-top: 8px; }
  .select-btn {
    margin-top: 12px; width: 100%; padding: 10px;
    background: var(--accent); color: #fff; border: none;
    border-radius: 8px; font-size: 14px; font-weight: 600;
    cursor: pointer; transition: opacity 0.2s;
  }
  .select-btn:hover { opacity: 0.85; }
</style>

<div class="grid">
  <!-- Repeat for each theme -->
  <div class="card">
    <div class="label">Boardroom</div>
    <div class="title-sample">Revenue grew 34%</div>
    <div class="body-sample">Every product line beat plan; hiring stayed flat.</div>
    <div class="swatches">
      <div class="swatch" style="background:#0F1B2A" title="bg"></div>
      <div class="swatch" style="background:#17263A" title="surface"></div>
      <div class="swatch" style="background:#4F9CF9" title="accent"></div>
      <div class="swatch" style="background:#3FBF8F" title="positive"></div>
      <div class="swatch" style="background:#E4695E" title="negative"></div>
    </div>
    <div class="badge">Dark · projector-safe · executive</div>
    <button class="select-btn" onclick="sendPrompt('Theme: Boardroom')">Select Boardroom</button>
  </div>
  <!-- ... more theme cards ... -->
</div>

<script>
function sendPrompt(p) {
  window.parent.postMessage({ type: 'widget_send_prompt', prompt: p }, '*');
}
</script>
```

### Slide grid template

```html
<style>
  :root { --bg: #FBFAF7; --surface: #F1EEE7; --title: #1C2430; --body: #2E3947; --muted: #5C6878; --accent: #1F5C8B; }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--body); padding: 16px; }
  .slide-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; }
  .slide-card {
    background: var(--surface); border-radius: 8px; overflow: hidden;
    border: 1px solid rgba(0,0,0,0.06);
  }
  .slide-header {
    background: var(--accent); color: #fff; padding: 6px 10px;
    font-size: 11px; font-weight: 600; display: flex; justify-content: space-between;
  }
  .slide-body { padding: 10px; }
  .slide-title { font-size: 13px; font-weight: 700; color: var(--title); margin-bottom: 6px; line-height: 1.3; }
  .slide-meta { font-size: 10px; color: var(--muted); }
  .slide-visual { font-size: 10px; color: var(--accent); margin-top: 4px; font-style: italic; }
  .summary {
    margin-top: 16px; padding: 12px; background: var(--surface);
    border-radius: 8px; font-size: 13px; color: var(--body);
    display: flex; justify-content: space-between; align-items: center;
  }
  .ghost-pass { color: #2E7D32; font-weight: 600; }
  .ghost-fail { color: #C62828; font-weight: 600; }
</style>

<div class="slide-grid">
  <!-- Repeat per slide -->
  <div class="slide-card">
    <div class="slide-header">
      <span>Slide 1</span>
      <span>cover</span>
    </div>
    <div class="slide-body">
      <div class="slide-title">Q3 Revenue grew 34% on 22% headcount</div>
      <div class="slide-visual">[stat callout + accent bar]</div>
      <div class="slide-meta">layout: title-only</div>
    </div>
  </div>
  <!-- ... more slides ... -->
</div>

<div class="summary">
  <span>10 slides · <span class="ghost-pass">Ghost deck: PASS</span></span>
  <span>Theme: Ledger</span>
</div>
```

## Agent integration

### Modified pipeline (when preview is triggered)

```
Phase 0  Read
Phase 1  Confirm theme + imagery
Phase 2  Outline with action titles
Phase 2.5  [NEW] Preview
  2.5a  visualize_read_me(modules=["interactive", "mockup"])
  2.5b  show_widget(theme_picker)  →  user selects theme
  2.5c  show_widget(slide_preview_grid)  →  user confirms or requests changes
  2.5d  [optional] show_widget(style_comparison)  →  user picks approach
Phase 4  Build
Phase 5  Verify
Phase 6  Hand off
```

### Fallback (when show_widget is unavailable)

The pipeline continues exactly as today: text-based outline approval via
`ask_user`, then build. No regression.

### Interaction flow

1. Agent calls `visualize_read_me(modules=["interactive", "mockup"])`
2. Agent builds theme card HTML from the 2-3 recommended themes
3. Agent calls `show_widget(title="theme_picker", widget_code=...)`
4. User clicks a "Select" button inside the widget
5. Widget sends `postMessage({ type: 'widget_send_prompt', prompt: 'Theme: Ledger' })`
6. Parent `WidgetRenderer` receives the message and calls `sendMessage(prompt)`
7. Agent receives the user message "Theme: Ledger" and proceeds
8. Agent builds slide grid HTML from the outline + selected theme
9. Agent calls `show_widget(title="slide_preview", widget_code=...)`
10. User reviews, optionally sends a follow-up message requesting changes
11. Agent adjusts outline if needed, rebuilds preview, or proceeds to build

### Streaming considerations

The HTML must be structured for progressive rendering:
- `<style>` block first (renders immediately, no flash)
- Content structure next (cards appear as chunks arrive)
- `<script>` block last (interactivity only after content is visible)

Keep total HTML under ~8KB for responsive streaming. Theme cards: ~2KB each.
Slide cards: ~400 bytes each. A 12-slide grid: ~5KB total.

## Constraints

| Constraint | Value | Notes |
|-----------|-------|-------|
| Max widget width | 1200px | show_widget parameter limit |
| Initial height | 150-900px | Auto-resizes via postMessage |
| Max content height | 2400px | WidgetRenderer clamp |
| HTML chunk size | 500 chars | Streaming simulation |
| Scripts | CDN only | Allowed CDNs per visualize guidelines |
| Dark mode | CSS variables | Must use `var(--color-*)` pattern |

## Risks

| Risk | Mitigation |
|------|-----------|
| HTML preview ≠ pptx output (font rendering, spacing differ) | Always label as "visual preview, not pixel-accurate" |
| Theme selection via sendPrompt is unstructured text | Agent parses the prompt string to extract theme name |
| Large slide counts (>15) make grid too tall | Paginate or show first 8 + "..." indicator |
| User ignores preview and wants to skip | Allow skip with "skip preview, just build" message |
| sendPrompt not wired (onSendPrompt undefined) | WidgetRenderer falls back to sendMessage via team store |

## Verification

1. Generate theme picker HTML, call `show_widget`, confirm cards render
2. Click a select button, confirm message arrives in chat
3. Generate slide grid HTML, call `show_widget`, confirm slide cards render
4. Verify ghost deck summary line is accurate
5. Confirm pipeline falls back to text flow when show_widget is absent
6. Check HTML size stays under 8KB for a 12-slide deck
