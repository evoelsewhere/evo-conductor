# Design

DES-020 follows the normal lifecycle. DES-011 is an explicitly labelled pre-approval planning draft
requested to prepare the next implementation slice.

## When to create a design

A `DES-NNN` is created only after `REQ-NNN` has moved to `Accepted`.

## How to create one

1. Copy [../base/TEMPLATE-DESIGN.md](../base/TEMPLATE-DESIGN.md) to `DES-NNN-<slug>.md`, using the same
   number as the requirement.
2. Fill in the metadata table and point `Requirement` at `REQ-NNN`.
3. Section 12, traceability from acceptance criteria to components, and section 13, task breakdown, are
   mandatory. Every acceptance criterion in the requirement must be claimed by at least one task.
4. Update the `Design` field in the requirement document.
5. Update the register below and in [../README.md](../README.md).

Tasks are created in [../task/](../task/) only after the design is approved.

## Register

| Step | ID | Requirement | Title | Created | Status |
|---|---|---|---|---|---|
| 01 | [DES-020](01-DES-020-automated-testing-ci.md) | REQ-020 | Automated testing and CI | 2026-08-09 | Draft |
| 12 | [DES-011](12-DES-011-client-registration.md) | REQ-011 | Client registration and connection | 2026-08-10 | Draft (planning) |
