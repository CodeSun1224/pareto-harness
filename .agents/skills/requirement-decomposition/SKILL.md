---
name: requirement-decomposition
description: Decompose Pareto Harness roadmap outcomes and epics into ordered, independently verifiable requirements. Use when planning a milestone, refining backlog, splitting an oversized requirement, or sequencing foundational and experimental capabilities.
---

# Decompose an epic

1. State the user-visible or engineering outcome and measurable Epic exit criteria.
2. Split by vertical behavior, not by technical layer or empty package. Each Requirement must produce observable value or prove one architectural invariant.
3. Keep a Requirement small enough for one coherent Spec, implementation, test matrix, and Review.
4. Identify prerequisite contracts and order them before consumers. Put Event/Revision/Capability/Evidence skeletons before features that need audit or recovery.
5. Separate baseline capability from experimental optimization. First make behavior correct and measurable; then optimize Task, Context, Router, Memory, or evolution independently.
6. Assign risk class, dependencies, acceptance summary, planned evidence, and milestone to every Requirement.
7. Avoid circular dependencies and "integration at the end"; require a runnable vertical slice in every milestone.
8. Update the Epic and Requirement Backlog, using stable IDs and links rather than copying full specifications.

Target 3-10 Requirements per Epic. Split a Requirement when it has multiple independent rollout decisions, unrelated risks, or cannot be reviewed as one coherent diff.
