# REQ-0034 Handoff

Current phase: remediation of REVIEW-0018. Trusted baseline is `origin/main` exact `e7a939cad71a85ada97c3b60d61ba5c024d85ab9` after REQ-0009 closure. The incomplete adapter-first REQ-0010 attempt is preserved at `archive-req0010-adapter-first-20260905` exact `fea9080fcc5529f4bdc1edf10e0bb4c5fc19f0cd` and is not releaseable or an implementation source.

The candidate introduces REQ-0034/SPEC-0010/RFC-0013 and will add planned REQ-0035/REQ-0036 while preserving all existing Requirement IDs. It moves REQ-0018 Plan/DAG and REQ-0016 minimal Evidence Gate before REQ-0014, keeps Memory non-authoritative, and separates procedure rollback, behavior rollback, Run recovery, Workspace recovery and Effect reconciliation/compensation.

No product code, Cargo, SQLite or Schema change is authorized. After an exact candidate commit, a fresh independent Reviewer must write only REVIEW-0018. Any open Blocker/Major keeps the route unaccepted. Even after route approval, REQ-0010 must be separately rewritten and independently approved before implementation.

First review exact `cfdc65af64675b8066b9bc429fbf998d588231bc` is `changes-requested` with F-001 Blocker and F-002/F-003/F-004 Major. The designer owns remediation; only the same Reviewer may close them. REVIEW-0018 original finding text and verdict remain preserved in commit `72a0f8b`.

Remediation content is exact `499116a8e93e00a737f0c112d0a0104eb9386840`. Validation and task metadata are being bound in a follow-up evidence commit; the same Reviewer must inspect both exact revisions and may update only formal Review documents.
