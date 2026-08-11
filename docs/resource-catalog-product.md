# Product design — governed resource catalog

Status: **MVP implemented in Conductor**

## Product problem

EvoFlux resources currently behave like files/configuration that members can pull, but a project needs a control plane that answers five operational questions:

1. What agents, skills and MCP servers are approved for use?
2. Which exact version is active, and what changed?
3. Who is allowed to receive each resource?
4. Who actually uses it, and does it work reliably?
5. What should the owner improve in the next version?

Conductor owns governance and measurement. EvoFlux remains the execution runtime and reports outcomes using a connection secret.

## Personas and jobs

| Persona | Job to be done |
|---|---|
| Admin | Govern the entire catalog, access boundaries and auditability |
| Contributor/resource owner | Create, version, publish and improve resources they own |
| Member | Discover permitted published resources and provide actionable feedback |
| Engineering/operations | Understand adoption, reliability, latency and reporting health |

## Product loop

```mermaid
flowchart LR
    D["Create draft"] --> P["Publish version"]
    P --> A["Resolve member access"]
    A --> U["Use in EvoFlux"]
    U --> M["Report outcome metrics"]
    M --> F["Review feedback and monitoring"]
    F --> D
```

The unit of governance is a stable **resource**. The unit of delivery is an immutable **resource version**.

## MVP scope

### Catalog management

- Resource kinds: agent, skill, plugin, workflow and command.
- Stable metadata: slug, name, description, owner and visibility.
- Lifecycle: `draft → published → archived`.
- Draft resource creation with an initial semantic version and file bundle stored in project object storage.
- Metadata editing without rewriting version history.
- Archive instead of destructive delete; history and monitoring remain queryable.

### Version governance

- Multiple immutable version records per resource.
- Version lifecycle: `draft → published → deprecated`.
- Exactly one published version per resource.
- Publishing creates an immutable content-addressed ZIP and atomically advances the selected release channel.
- SQL stores only object keys, digests, sizes and manifests; authored files and logos use Local, S3, Azure Blob or Git storage.
- Changelog required by product workflow, optional at API level for migration compatibility.

### Access model

- Admin can manage every resource.
- Contributor can create resources and manage/publish only resources they own.
- User consumes only published resources that match access policy.
- Shared resource with no explicit policy defaults to all active members.
- Private resource with no explicit policy defaults to owner only.
- Explicit allow subjects: all members, primary roles, sub-roles, member tags and individual members.
- Admin and owner retain access so a policy cannot orphan its resource.

The MVP intentionally supports allow rules only. Deny rules and nested policy expressions are deferred because they make policy evaluation and support substantially harder.

### Effectiveness monitoring

- EvoFlux sends idempotent batches of usage outcomes.
- Member identity is derived from the connection-secret owner, never accepted from the request body.
- Measures: executions, successes, failures, success rate, duration, tokens and active members.
- Views: daily execution chart and per-member adoption table for 7, 30 or 90 days.
- Raw event IDs prevent retry duplication.

### Portfolio analytics and saved views

- Analytics Studio composes ordered KPI, line, area, bar, stacked-bar, donut and table widgets from an allowlisted metric/dimension catalog.
- Dashboard definitions persist layout, density, relative/custom date range, comparison mode and typed telemetry filters. They cannot contain SQL, expressions or arbitrary query keys.
- Every view is project-scoped and owner-attributed. `private` views are visible to their owner and project admins; `shared` views are readable by telemetry-capable project contributors.
- Contributors can change only views they own. Project admins can audit and manage every project view.
- Updates and deletes require the last-read revision; stale writers receive `409 Conflict` instead of overwriting a newer dashboard.
- The console API is `GET/POST /api/analytics/views` plus `GET/PUT/DELETE /api/analytics/views/{id}`. `PUT` is a complete replacement and `DELETE` carries `?revision=<last-read>`.

### Feedback

- One current rating/comment per member and resource.
- A later submission updates the member's prior feedback.
- Feedback is associated with the published version at submission time.
- Owners/admins see member feedback and aggregate rating.

## Permission matrix

| Capability | Admin | Contributor owner | Contributor non-owner | User |
|---|---:|---:|---:|---:|
| List accessible published resources | ✓ | ✓ | ✓ | ✓ |
| List all drafts/archived resources | ✓ | Own only | — | — |
| Create resource | ✓ | ✓ | — | — |
| Edit metadata/access | ✓ | Own only | — | — |
| Create/publish version | ✓ | Own only | — | — |
| Archive | ✓ | Own only | — | — |
| View monitoring/member feedback | ✓ | Own only | — | — |
| Read shared analytics views | ✓ | ✓ | ✓ | — |
| Create/update own analytics views | ✓ | ✓ | ✓ | — |
| Manage another member's analytics view | ✓ | — | — | — |
| Submit feedback | When accessible | When accessible | When accessible | When accessible |

## Success metrics

Product health should be reviewed using:

- Catalog adoption: published resources used by at least one member in 30 days.
- Active adoption: unique members per resource and per version.
- Reliability: success rate and failure count.
- Performance: average execution duration and token volume.
- Feedback quality: response rate and average rating.
- Governance hygiene: drafts older than 30 days, published versions without changelog, archived-but-still-reported usage.

The MVP exposes resource-level and portfolio-level analytics. Governance hygiene reports remain the next product slice.

## Data and privacy decisions

- Usage stores operational metadata, not prompts, responses or tool arguments.
- Saved views store only typed presentation/query configuration; they never store telemetry results, prompt content or executable query text.
- `user_id` comes from authenticated connection context.
- Event time is accepted only within a bounded 90-day window and five-minute future skew.
- Batch size is limited to 100; payload and text fields are bounded.
- Archive preserves audit history.
- Production must define raw-event retention before broad rollout; recommended initial retention is 90 days followed by daily aggregates.

## Out of scope for this MVP

- Marketplace discovery across projects.
- Approval workflow requiring a second reviewer.
- Rollback button; publishing an older payload as a new version is the safe interim process.
- Deny/conditional access rules.
- Cost calculation tied to model-provider price tables.
- Prompt/response capture or qualitative trace inspection.
- Distributed event bus/outbox for multiple Conductor replicas.
- EvoFlux-side implementation.

## Next roadmap

1. Governance hygiene: stale drafts, unused releases and unresolved delivery failures.
2. Approval gates and separation of author/publisher for regulated projects.
3. Version comparison, rollback-as-new-version and release notes.
4. Daily aggregate jobs and retention controls.
5. Transactional outbox + NATS JetStream for multi-replica delivery.
6. EvoFlux inventory reconciliation and client version compliance.
