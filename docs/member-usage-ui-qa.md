# Member usage and activity UI — QA evidence

Validated on 2026-08-10 against the local Conductor API and a seeded SQLite QA database.

## Acceptance checklist

- [x] Member rows navigate to `/app/members/:userId` with mouse, Enter, and Space.
- [x] Member overview shows request, token, cost, latency, error-rate, and tool-call KPIs.
- [x] Usage charts render token trends and model/provider distribution.
- [x] Tools view renders ranked tool usage with date-range filtering.
- [x] Activity view filters request audit rows and opens request details.
- [x] Request details show model, provider, tokens, duration, cost, status, and tool events.
- [x] Desktop and 390 px mobile layouts render without horizontal overflow.
- [x] Provider icons use local SVG assets and retain an accessible text fallback.
- [x] Browser console remained free of runtime errors during the Playwright flows.

## Automated verification

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace                         # 42 passed
bun run typecheck
bun run build
Playwright smoke/visual flows                  # passed
```

The Vite production build reports only its existing large-chunk and `__dirname` compatibility warnings.

## Screenshots

### Member overview — desktop

![Member overview on desktop](assets/member-usage-ui/member-overview-desktop.jpg)

### Activity filtering — desktop

![Filtered member activity on desktop](assets/member-usage-ui/member-activity-filtered-desktop.jpg)

### Request audit detail — desktop

![Member request audit detail on desktop](assets/member-usage-ui/member-request-detail-desktop.jpg)

### Tool usage — desktop

![Member tool usage on desktop](assets/member-usage-ui/member-tools-desktop.jpg)

### Member overview — mobile

![Member overview on mobile](assets/member-usage-ui/member-overview-mobile.jpg)

### Charts — mobile

![Member charts on mobile](assets/member-usage-ui/member-charts-mobile.jpg)
