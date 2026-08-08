# TEMPLATE — Design

Copy this file to `design/DES-NNN-<slug>.md`. Only create it once `REQ-NNN` is `Accepted`.
Shared conventions live in [BASE-CONVENTIONS.md](BASE-CONVENTIONS.md).

---

# DES-NNN — \<Title matching REQ-NNN\>

| | |
|---|---|
| ID | DES-NNN |
| Created | YYYY-MM-DD |
| Updated | YYYY-MM-DD |
| Status | Draft |
| Requirement | [REQ-NNN](../requirement/REQ-NNN-\<slug\>.md) |
| References | [architecture.md](../architecture.md), [BASE-CONVENTIONS](../base/BASE-CONVENTIONS.md) |
| Tasks | Not created; requires approval |

## 1. Goal

<!-- Restate the acceptance criteria as short bullets. Link, do not copy. -->

## 2. Options considered

| Option | Advantages | Disadvantages | Outcome |
|---|---|---|---|
| A | | | Selected / Rejected |
| B | | | |

**Rationale for the selected option:**
<!-- Two or three sentences. This is the part a future reader needs most. -->

## 3. Data model changes

<!-- New tables, new columns, indexes. Include the migration SQL. State whether it is backward compatible. -->

```sql
```

**Migration of existing data:**
<!-- Required or not, how it is performed, how it is reversed. -->

## 4. API changes

| Method | Path | Authentication | Required role or scope | Description |
|---|---|---|---|---|
| | | session / token | | |

<!-- Include example request and response bodies, and an error-code table, for each new endpoint. -->

## 5. Backend changes

| Crate | File | Change |
|---|---|---|
| | | |

<!-- Respect the existing layering: domain, then storage, then auth, then server. -->

## 6. Frontend changes

| Route or screen | Component | State and data source |
|---|---|---|
| | | |

## 7. EvoFlux changes

<!-- State "not applicable" when the design does not touch EvoFlux. -->

## 8. Security and authorization

<!-- Who may call what. Where secrets are stored and in what form.
     Check against the privacy boundary in BASE-CONVENTIONS section 10. -->

## 9. Performance

<!-- Expensive queries, required indexes, expected data volume, acceptable thresholds. -->

## 10. Rollout and rollback

<!-- Release order, feature flags if any, how to reverse the change. -->

## 11. Test strategy

<!-- Which test type covers which behaviour. Tooling per BASE-CONVENTIONS section 8. -->

## 12. Traceability: acceptance criteria to components

| AC | Responsible component | Planned task |
|---|---|---|
| AC-1 | | TSK-NNN-01 |

## 13. Task breakdown

| Task | Layer | Description | Depends on |
|---|---|---|---|
| TSK-NNN-01 | BE | | none |
| TSK-NNN-02 | FE | | TSK-NNN-01 |

## History

| Date | Change | Author |
|---|---|---|
| YYYY-MM-DD | Created | |
