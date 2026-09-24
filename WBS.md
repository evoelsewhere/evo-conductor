# WBS — Conductor Phase 2 (đối chiếu task.md với src thực tế)

Nguồn: [task.md](task.md) (self-reported bởi team) đối chiếu với code trong repo tại `feature/conductor-phase-2` (2026-09-25).
Chú thích trạng thái:
- ✅ **Implemented** — có evidence rõ trong code (model/route/migration/UI).
- 🟡 **Partial** — có nền tảng nhưng thiếu phần task.md mô tả, hoặc chỉ generic chứ chưa đúng scope.
- ⬜ **Not started** — không tìm thấy code, chỉ là dòng trong task.md.

---

## 1. Standardize provider tool usage for each project

| WBS | Subtask | PIC | task.md status | **Code thực tế** | Evidence |
|---|---|---|---|---|---|
| 1.1 | Define standardized provider tool configurations | Thang | DONE 100% | ✅ Implemented | `ai_policy.rs` — `AiPolicyScope`, `UpsertAiPolicyRequest{allowed_providers, allowed_tools}` |
| 1.2 | Implement project-level tool configuration APIs | Hung | DONE 100% | ✅ Implemented | `POST/GET/DELETE /ai-policy`, `GET /v1/client/ai-policy` — `routes/mod.rs:828-855,1054-1061`, `routes/ai_policy.rs` |
| 1.3 | Create the project tool management interface (base) | Thang | DONE 100% | ✅ Implemented | `apps/web/src/features/ai-policy/pages/ai-policy-page.tsx` + `ai-policy-dialog.tsx` |
| 1.4 | Integrate and test tool configurations in project workflows | Hung | DONE 100% | 🟡 Partial | Allow-list là **free-text string** (`allowed_tools`/`allowed_providers` space-separated), chưa có catalog tool chuẩn (Claude Code/Cursor/...) để chọn — task.md gọi là DONE nhưng UX chưa đạt mức "standardize" thật sự |

**Việc còn lại (đề xuất bổ sung, chưa có trong task.md):**
- Curated catalog of known providers/tools (dropdown/multi-select thay free-text) để tránh sai chính tả và thật sự "standardize".
- Validate allow-list value against catalog ở backend (hiện không có ràng buộc).

---

## 2. Enable AI model configuration and usage monitoring at project level

| WBS | Subtask | PIC | task.md status | **Code thực tế** | Evidence |
|---|---|---|---|---|---|
| 2.1 | Define project-level AI model configs & usage metrics | Thang | DONE 100% | ✅ Implemented | model/provider default nằm trong `ai_policy` rows; usage ingest `routes/telemetry.rs` |
| 2.2 | Implement AI model configuration & usage tracking APIs | Hung | DONE 100% | ✅ Implemented | `core/model_pricing.rs`, `core/model_cost_report.rs`, tables `model_price_catalogs`/`model_prices` (`migrate.rs:485,496`) |
| 2.3 | Create the AI model configuration and usage dashboard | Hung | DONE 100% | ✅ Implemented | `apps/web/src/features/monitoring/pages/monitoring-page.tsx` (`member-cost-table.tsx`, `model-cost-table.tsx`), `spend/pages/spend-page.tsx` |
| 2.4 | Integrate and test model selection, token tracking, cost monitoring | Thang | DONE 100% | ✅ Implemented | Scheduled models.dev pricing sync — `main.rs:39,58-97` |

**Đây là nhóm hoàn thiện nhất, khớp với task.md.** Không có việc tồn đọng lớn — chỉ thiếu export dữ liệu (xem mục 5).

---

## 3. Enhance auth/authz — user-based permissions for tools and AI models

| WBS | Subtask | PIC | task.md status | **Code thực tế** | Evidence |
|---|---|---|---|---|---|
| 3.1 | Define user roles and permissions for tools/AI models | Thang | DONE 100% | 🟡 Partial | Role chỉ có 3 bậc **Admin/Contribute/User** với boolean capability (`role.rs:9-62`) — không có permission theo từng tool/model |
| 3.2 | Implement permission management and authorization APIs | Thang | DONE 100% | ✅ Implemented | `conductor-domain/src/authorization.rs` (1172 dòng, permission keys) |
| 3.3 | Create the user permission management interface | Hung | InProgress 50% | 🟡 Partial | `apps/web/src/features/roles/pages/roles-page.tsx` — có grid permission × role nhưng **read-only cho fixed role**, chỉ sub-role/tag mới CRUD được |
| 3.4 | Apply and test permission checks across configuration/execution flows | Hung | InProgress 50% | 🟡 Partial | Permission check tồn tại nhưng granularity dừng ở role/project (`ai_policy` scope), chưa per-member |

**Việc còn lại (khớp đúng phần task.md đang InProgress):**
- **3.3**: Cho phép edit permission ở granularity per-tool/per-model, không chỉ role-level boolean.
- **3.4**: Thêm permission check per-user (hiện `ai_policy` chỉ scope Project/Role, thiếu Member-level override).
- Cân nhắc: định nghĩa rõ "permission cho tool/AI model" là gì (task.md chưa tách bạch với permission chung của roles-page hiện tại).

---

## 4. Integrate usage tracking with Jira through Atlassian APIs

| WBS | Subtask | PIC | task.md status | **Code thực tế** | Evidence |
|---|---|---|---|---|---|
| 4.1 | Configure Atlassian API authentication and Jira connection | Thang | DONE 100% | ✅ Implemented | `core/jira.rs` — Basic Auth + API token, `test_connection()` (`/rest/api/3/myself`) |
| 4.2 | Aggregate project-level AI usage and cost data | Hung | InProgress 50% | ✅ Implemented | `render_report_adf` build report từ usage data (`jira.rs:231-284`) |
| 4.3 | Implement automatic Jira issue creation and updates | Hung | InProgress 30% | ⬜ Not started | `post_usage_report` chỉ **post comment vào 1 issue có sẵn** (`report_issue_key`), **không tạo issue mới** |
| 4.4 | Add scheduled synchronization, error handling, audit logs | Hung | InProgress 20% | 🟡 Partial | Scheduled sync thật (`spawn_jira_report_sync`, `main.rs:103-163`), error handling có, nhưng **audit log chỉ là `tracing::info!/warn!`**, không có bảng audit log persist |

**Việc còn lại (đúng như task.md ghi InProgress, xác nhận qua code):**
- **4.3**: Implement tạo Jira issue mới (hiện chỉ comment vào issue cấu hình sẵn) — cần API `POST /rest/api/3/issue`.
- **4.4**: Thêm bảng audit log (vd. `jira_sync_log`) để tra cứu lịch sử đồng bộ thay vì chỉ log runtime.

---

## 5. Define a standardized usage report format for Jira, Excel, PowerPoint

| WBS | Subtask | PIC | task.md status | **Code thực tế** | Evidence |
|---|---|---|---|---|---|
| 5.1 | Define standard usage metrics and reporting fields | Thang | DONE 100% | ✅ Implemented (định nghĩa) | Metrics đã có sẵn qua model_cost_report/telemetry, dùng chung cho Jira ADF report |
| 5.2 | Design unified report structure for Jira, Excel, PowerPoint | Thang | InProgress 10% | ⬜ Not started | Chỉ có Jira ADF text report (`jira.rs:231-284`); không có struct chung đa định dạng |
| 5.3 | Implement data mapping + report generation per format; recipients/delivery/schedule/timezone/cost alerts | Thang | InProgress 10% | ⬜ Not started | Không có dependency xlsx/pptx trong `Cargo.toml` hay `package.json`; không export csv/xlsx ở đâu trong UI |
| 5.4 | Validate report content/format with stakeholders | Thang | InProgress 10% | ⬜ Not started | Không thể validate vì chưa có report engine |

**Việc còn lại — đây là nhóm ít nhất, cần làm gần như từ đầu:**
- Chọn crate Rust cho xlsx (`rust_xlsxwriter`) và pptx (chưa có crate Rust ổn định phổ biến — cân nhắc build ở frontend qua thư viện JS, hoặc generate qua template).
- Thiết kế 1 struct report chung (metrics, period, project) rồi map ra 3 renderer: Jira ADF (đã có), Excel, PowerPoint.
- Thêm cấu hình: recipients, delivery channel (email/Jira/download), schedule + timezone, cost alert threshold — hiện chỉ có `spend_limits` (ngưỡng $ dashboard, không phải report delivery) và `jira.report_interval_hours` (single-target).
- **Phụ thuộc**: cần email sending đã có sẵn (`lettre` dependency, xem commit "Add lettre dependency") — có thể tái dùng cho report delivery qua email.

---

## 6. Build marketplace-ready tools and plugins

**Đã test thật (2026-09-25), không chỉ đọc code — xem mục "Cách đã test" cuối file.**

| WBS | Subtask | PIC | task.md status | **Code thực tế (đã test)** | Evidence |
|---|---|---|---|---|---|
| 6.1 | Plugin configuration, permissions, security controls | Thang | InProgress 10% | ✅ Implemented — **verified chạy được** | `cargo test --test plugin_archive_import`: 5/5 pass (`regular_members_cannot_inspect_plugin_packages`, `owning_contributor_can_author_plugin_draft_but_cannot_release_or_archive_it`, ...). UI: nút "Validate" chạy thật trên plugin `jira-task-assistant` → *"Draft revision 1 passes static validation."* |
| 6.2 | Plugin installation, update, version management | Thang | InProgress 10% | ✅ Implemented — **verified chạy được** | UI tab "Versions (1)" hiển thị dữ liệu thật: `v0.1.0`, lifecycle `Active Published`, integrity hash `13ac1bb229a4...`, nút Restore/Deprecate hoạt động (immutable release history) |
| 6.3 | GitHub/S3 storage config + CI/CD pipeline for build/test/package/release | Thang | InProgress 10% | 🟢 Storage(S3): Implemented & fixed / 🟡 Storage(GitHub): generic Git only / ⬜ CI/CD: Not started | **Cập nhật 2026-09-25**: đã fix + verify S3 storage credentials (xem "Fix S3 credentials" bên dưới) — không còn là gap. `StorageBackend` enum (`instance.rs:156-162`) vẫn chỉ `Local\|S3\|AzureBlob\|Git` — Git generic dùng được với repo GitHub qua HTTPS token, nhưng không có backend/API tích hợp "GitHub" riêng. **CI/CD: xác nhận lại lần 2 bằng `find .github` → vẫn không tồn tại, không có pipeline file nào trong repo — phần này hoàn toàn chưa động tới** |

**Phát hiện bất ngờ khi test — có 1 plugin thật đã tồn tại trong hệ thống:**
Khi mở `/app/resources/plugins` trên instance đang chạy, đã thấy sẵn plugin **"Jira Task Assistant"** (Published, v0.1.0, do "TienHN" tạo lúc 25/09/2026 00:27 — trùng PIC "Dang/Tien" ở mục 7 task.md). Đây chính là minh chứng sống cho mục 6 hoạt động thật: plugin này được author, validate, và release **qua đúng pipeline 6.1+6.2** đang audit. Chi tiết plugin này xem mục 7 bên dưới (audit lần trước cho mục 7 là **sai** — cần đính chính).

**Việc còn lại:**
- **6.3 — CI/CD**: Vẫn thiếu nhiều nhất trong nhóm 6, **chưa được động tới trong đợt fix S3 vừa rồi** — cần dựng CI/CD pipeline thật (GitHub Actions hoặc tương đương) cho build/test/package/release plugin, hiện chưa có 1 file nào.
- **6.3 — GitHub backend riêng**: nếu task.md ý muốn tích hợp GitHub API chuyên biệt (không chỉ Git generic qua HTTPS token) thì vẫn chưa có — cần làm rõ với Thang liệu Git generic đã đủ hay cần backend GitHub riêng.
- Marketplace discovery/storefront (list plugin công khai, rating...) — chưa có ở đâu ngoài `docs/resource-catalog-product.md` (chỉ là tài liệu thiết kế, chưa code).

### Cập nhật 2026-09-25 — Fix S3 credentials (giải quyết 1 phần của 6.3)

Trong lúc user test object storage thật, phát hiện lỗi: chọn backend S3 báo `storage migration failed: Generic S3 error: ... PUT http://169.254.169.254/latest/api/token ... timeout`. Nguyên nhân: UI Object Storage không có ô Access Key ID/Secret Access Key, server chỉ dựa vào AWS credential chain (env var/IAM role) nên cố dò qua AWS EC2 metadata service (IMDS) và timeout khi không chạy trên EC2.

**Đã sửa** (theo đúng pattern bảo mật sẵn có — Jira token/Git HTTPS token: file write-only riêng ngoài SQL):
- [`crates/conductor-domain/src/instance.rs`](crates/conductor-domain/src/instance.rs) — thêm `access_key_id`, `secret_access_key` (write-only), `secret_access_key_set` vào `S3StorageSettings`.
- [`crates/conductor-server/src/core/artifacts.rs`](crates/conductor-server/src/core/artifacts.rs) — generic hóa cơ chế rollback-safe credential (trước chỉ dành Git) dùng chung cho cả S3.
- [`apps/web/src/features/settings/components/settings-form.tsx`](apps/web/src/features/settings/components/settings-form.tsx) — thêm ô Access Key ID + Secret Access Key (write-only, "leave blank to keep").

**Đã test:**
- `cargo test -p conductor-server` toàn bộ — **41/41 test suite pass, 0 fail**.
- Gọi thẳng `PUT /api/settings/storage` với endpoint thật của user (`https://aws.fmate.id.vn`) — lỗi đổi từ timeout IMDS sang lỗi HTTP thật từ endpoint đó, xác nhận credentials được dùng đúng.
- Revert về `local` qua API — không phá luồng cũ.

**Vậy 6.3 giờ chỉ còn thiếu CI/CD pipeline (~90% còn lại của mục này), phần storage credentials đã xong.**

### Cập nhật 2026-09-25 (tiếp) — CI/CD pipeline + làm rõ "GitHub backend"

**CI/CD — đã tạo mới hoàn toàn (trước đó = 0 file):**
- [`.github/workflows/ci.yml`](.github/workflows/ci.yml) — chạy trên mọi push/PR: job `rust` (`cargo check` + `cargo test --workspace`), job `web` (`bun install`, `typecheck`, `test:unit`, `build`), job `docker` (build thử image từ `Dockerfile` sẵn có để bắt lỗi build sớm).
- [`.github/workflows/release.yml`](.github/workflows/release.yml) — chạy khi push tag `v*.*.*`: build + push image lên GHCR (`ghcr.io/<repo>`, dùng `GITHUB_TOKEN` có sẵn, không cần secret ngoài), rồi tạo GitHub Release kèm auto-generated notes. Đây chính là bước "package/release" task.md yêu cầu — tái dùng `Dockerfile` multi-stage đã có (web dist + Rust release binary) thay vì viết lại pipeline đóng gói từ đầu.
- Phạm vi: pipeline này build/test/package/release cho **chính codebase Conductor** (server + web) — vì plugin cá nhân (mục 7) sống trong resource-store runtime của Conductor (DB + object store), không phải file trong git repo, nên không có "mỗi plugin 1 lần CI trigger qua git push" theo nghĩa truyền thống. Việc "build/test" cho plugin đã có ở lớp khác: nút **Validate** trong UI (đã test thật, xem mục 6.1) đóng vai trò CI cho từng plugin package.
- **Chưa test chạy thật trên GitHub** (cần push lên remote GitHub thật + có Actions bật) — chỉ review cú pháp YAML thủ công, chưa chạy qua `act` hay tương đương. Cần verify khi push commit đầu tiên.

**"GitHub backend riêng" — quyết định: không xây API backend mới, mở rộng Git backend sẵn có (đã test hoạt động với GitHub qua HTTPS token — xem test `admin_migrates_objects_to_git_without_exposing_credentials`):**
- Lý do: xây 1 `StorageBackend::GitHub` riêng dùng GitHub REST/Contents API sẽ trùng lặp hoàn toàn với Git backend hiện có (cùng mục đích: lưu resource object trong 1 repo GitHub), chỉ khác cơ chế (API call thay vì `git` CLI) — không có giá trị thêm rõ ràng trừ khi cần tính năng GitHub-only (Releases, GitHub App, webhook), điều task.md không yêu cầu cụ thể.
- Đã làm: cập nhật UI — đổi label dropdown "Git repository" → "Git repository (GitHub, GitLab, self-hosted)", thêm hint cụ thể cách dùng với GitHub (URL mẫu `https://github.com/org/repo.git`, PAT fine-grained scope `Contents: read/write`, gợi ý username `x-access-token`) — [`settings-form.tsx`](apps/web/src/features/settings/components/settings-form.tsx). Backend không cần đổi (`validate_git_repository_url` đã generic, không chặn domain nào).
- Nếu cần một backend GitHub API thật sự riêng biệt sau này (vd. để dùng GitHub Releases thay vì object storage, hoặc GitHub App thay PAT) — đây là việc mới, cần chốt scope riêng với Thang trước khi làm, không nằm trong phạm vi đã hoàn thành hôm nay.

---

## 7. Propose & develop 2 practical tools/plugins publishable on marketplace

> ⚠️ **Đính chính so với audit trước**: lần audit đầu chỉ grep git source tree (`crates/`, `apps/web/src`) nên báo "0 code artifact". Nhưng plugin của nền tảng này **không sống trong git repo** — nó là "Resource" được author và lưu trong resource-store runtime của chính Conductor (content-addressed object store dưới `data/data/objects/`). Đã test trực tiếp trên instance đang chạy và xác nhận plugin **có thật, hoạt động được**.

| WBS | Subtask | PIC | task.md status | **Code thực tế (đã test)** | Evidence |
|---|---|---|---|---|---|
| 7.1 | Jira Task Assistant plugin (retrieve/search/filter Jira task, status/assignee/deadline/missing info) | Dang/Tien | InProgress 60% | ✅ **Implemented & validated**, gần đúng 60-80% | Xem chi tiết bên dưới |
| 7.2 | Tool/plugin thứ 2 (chưa đặt tên trong task.md) | ? | *(chưa có dòng trong task.md)* | ⬜ Not started | Không tìm thấy plugin thứ 2 nào khác trong Catalog (chỉ có 1 plugin "Published") |

### Chi tiết 7.1 — đã test trực tiếp qua UI + API

Plugin `jira-task-assistant` (project `evo-conductor`, resource id `5186ab1b-...`), Published v0.1.0, tạo bởi author "TienHN" lúc 25/09/2026 00:27, gồm 8 file:

| File | Nội dung xác nhận |
|---|---|
| `plugin.json` | Schema `agent-plugins.org/1.0.0` hợp lệ, khai báo credential fields `jira_base_url`/`jira_email`/`jira_api_token` |
| `mcp.json` | Khai báo MCP server stdio: `python bin/jira-server.py`, truyền env credentials |
| `bin/jira-server.py` (309 dòng) | MCP JSON-RPC server thật qua stdin/stdout, implement đủ 4 tool: `jira_get_issue`, `jira_search_issues` (JQL), `jira_get_assignee`, `jira_check_missing_fields` — đúng scope "retrieve/search/filter/assignee/deadline/missing info" task.md yêu cầu |
| `tests/test_jira_mcp.py` | Test structure + SKILL.md frontmatter (chỉ test file tồn tại, chưa test gọi Jira API thật — không có mock HTTP) |
| `skills/jira-task-assistant/SKILL.md`, `README.md`, `LICENSE` | Đầy đủ |

**Đã test bằng nút "Validate" thật trên UI** → kết quả: *"Draft revision 1 passes static validation."*
**Đã test tab "Versions"** → `v0.1.0` Active/Published, integrity hash xác nhận immutable release đã chạy qua pipeline 6.1/6.2 thật.

**Còn thiếu so với "60%" task.md claim / để hoàn thiện 100%:**
- `test_jira_mcp.py` chưa test logic thật của 4 tool (get_issue/search/assignee/missing_fields) — chỉ test file tồn tại. Cần thêm unit test có mock HTTP cho `JiraClient`.
- Chưa verify plugin chạy thật với 1 Jira instance thật (chỉ mới static-validate cấu trúc, chưa smoke-test runtime MCP stdio).
- `jira_search_issues` gọi `/rest/api/3/search/jql` — cần double-check endpoint này đúng với Jira Cloud hiện hành (Atlassian đã đổi từ `/rest/api/3/search` sang `/search/jql` gần đây, cần xác nhận version phù hợp).

**Việc còn lại:**
- **7.1**: Đánh giá lại % — không phải 0% (audit cũ sai) nhưng cũng chưa hẳn "chạy production" — đề xuất mức **~70%**: cần thêm test logic thật + smoke test với Jira instance thật trước khi công bố marketplace.
- **7.2**: task.md chưa định nghĩa plugin thứ 2 — cần đề xuất & chốt scope trước khi ước lượng effort.

---

## Tổng kết mức độ sẵn sàng

| Nhóm | task.md claim (trung bình) | Code thực tế |
|---|---|---|
| 1. Provider tool standardization | ~100% | 🟡 ~75% (thiếu catalog chuẩn) |
| 2. AI model config + monitoring | 100% | ✅ ~100% |
| 3. Roles/permissions tools & AI model | ~75% | 🟡 ~50% (coarse-grained) |
| 4. Jira usage sync | ~75% | 🟡 ~60% (thiếu issue creation + audit log) |
| 5. Standardized report (Jira/Excel/PPT) | ~30% | ⬜ ~10% (chỉ có Jira text report) |
| 6. Marketplace-ready plugins | ~10% | ✅ 6.1/6.2 verified chạy được; ⬜ 6.3 CI/CD = 0 → tổng ~65% |
| 7. 2 plugin thực tế | 60% (1 plugin) | ✅ 7.1 verified chạy được (~70%), ⬜ 7.2 = 0% |

**Nhận xét chính:**
- Nhóm 1–2 (provider tool + AI model monitoring) đã production-ready, khớp task.md.
- **Nhóm 6 & 7 đã được test lại trực tiếp (2026-09-25) và kết quả tốt hơn nhiều so với audit grep ban đầu**: plugin authoring/validate/version-release (6.1, 6.2) chạy thật (test pass + UI Validate pass), và plugin Jira Task Assistant (7.1) là code thật, không phải placeholder — audit đầu tiên (chỉ grep git source) đã bỏ sót vì plugin sống trong resource-store runtime chứ không phải trong git repo.
- Nhóm 5 vẫn là **rủi ro lớn nhất còn lại**: task.md báo ~10-30% nhưng code = gần như 0%, chưa test lại vì chưa có gì để test.
- Nhóm 3–4 tiến độ thực tế thấp hơn self-report, đúng hướng InProgress nhưng cần làm rõ thêm phần "fine-grained per-tool/model" và "issue creation".
- Nhóm 6.3 (CI/CD) và 7.2 (plugin thứ 2) là phần thiếu rõ ràng nhất, xác nhận bằng kiểm tra trực tiếp filesystem/UI, không chỉ suy đoán.

---

## Cách đã test nhóm 6 (Build marketplace-ready tools and plugins)

Không chỉ đọc code — đã chạy 3 lớp kiểm tra thật trên instance đang sống (`http://127.0.0.1:4700` API + `:5174` web, build từ chính branch này):

1. **Automated test suite có sẵn trong repo**:
   ```
   cargo test -p conductor-server --test plugin_archive_import -- --nocapture
   ```
   → **5/5 test pass**: quyền hạn (contributor không release/archive được), reject archive không hợp lệ, atomic draft creation, revision conflict handling, member thường không inspect được package. Đây là bằng chứng 6.1 hoạt động đúng thiết kế bảo mật.

2. **Kiểm tra filesystem trực tiếp** cho CI/CD (6.3):
   ```bash
   find .github -type f   # → không tồn tại
   find . -iname "*.yml" -o -iname "*.yaml"   # → không có pipeline nào ngoài docker-compose
   ```
   → xác nhận chắc chắn CI/CD = 0, không phải suy đoán từ grep code.

3. **Test qua UI thật (Browser pane)** — đăng nhập admin, vào `Resources → Plugins`, thấy sẵn 1 plugin `Jira Task Assistant`:
   - Bấm nút **"Validate"** thật → server trả về *"Draft revision 1 passes static validation"* (xác nhận pipeline validate plugin.json/mcp.json hoạt động thật, không phải mock).
   - Mở tab **"Versions"** → thấy `v0.1.0 Active Published` với integrity hash thật, nút Restore/Deprecate — xác nhận version/release lifecycle (6.2) hoạt động.
   - Gọi trực tiếp API `GET /api/resources/{id}/draft/files` (qua `fetch()` trong console) để lấy **toàn bộ nội dung 8 file thật** của plugin (không phải chỉ tên file) — đọc source code `jira-server.py` (309 dòng) để xác nhận đây là MCP server thật, không phải file rỗng/placeholder.

**Vì sao cách này đáng tin hơn chỉ đọc code:** báo cáo audit lần đầu (chỉ `grep`/`find` trong git source tree) đã cho kết luận sai ở mục 7 ("Not started") vì không biết rằng plugin của nền tảng này được lưu trong resource-store runtime (DB + content-addressed object store ở `data/data/objects/`), không phải file trong git repo. Test qua UI + API thật mới lộ ra điều đó.

---
*Tạo bởi Claude Code — đối chiếu `task.md` (2026-09-25) với source tại nhánh `feature/conductor-phase-2`, cập nhật sau khi test trực tiếp nhóm 6.*
