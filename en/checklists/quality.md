# Requirements Quality Checklist: FreeSpace

**Purpose**: Verify the completeness, clarity, consistency and testability of requirements before implementation
**Created**: 2026-11-15
**Feature**: [spec.md](../spec.md)

**Note**: A custom checklist created by the `/speckit.checklist` command based on the specification, plan and
contracts. This is a reviewer artifact: the items verify the quality of requirements, not the operation of code.
**Review Ownership**: Mark an item `[x]` only when the reviewer has confirmed that the requirement quality
criterion is met.
**Marker Semantics**: `[x]` means verified and satisfied requirement quality, not
completed implementation.

## Requirement Completeness

- [ ] CHK001 Are all 23 user stories defined with priorities and independent verification criteria? [Completeness, Spec §User Scenarios]
- [ ] CHK002 Are all features from the original description covered by requirements (MFT, treemap, shredder, snapshots, archives, CLI, permissions)? [Completeness, Spec §Requirements]
- [ ] CHK003 Are explicit v1 scope boundaries defined (network, cloud, file recovery, cross-platform)? [Completeness, Spec §Assumptions]
- [ ] CHK004 Are requirements for empty states, loading and errors defined for each main screen? [Gap]
- [ ] CHK005 Are logging and cancellation requirements defined for all destructive operations? [Completeness, Spec §FR-042–FR-046]
- [ ] CHK006 Are requirements specified for full localization of all user strings? [Completeness, Spec §FR-092]

## Requirement Clarity

- [ ] CHK007 Is the "8 TB ≤ 60 s" figure quantified with environment conditions (NTFS, SSD, permissions, file count)? [Clarity, Spec §FR-003, SC-001]
- [ ] CHK008 Is it unambiguously defined which objects are considered inaccessible and how they affect the final totals? [Clarity, Spec §FR-006]
- [ ] CHK009 Are logical size, physical size and size on disk distinguished in all sections and reports? [Clarity, Spec §FR-005, data-model §3]
- [ ] CHK010 Is the concept of a "stale file" defined, taking into account a disabled last access date? [Clarity, Spec §FR-034]
- [ ] CHK011 Are the rules for automatically selecting which duplicate copy to keep unambiguously defined? [Clarity, Spec §FR-028]
- [ ] CHK012 Are the path length limit and the behavior when it is exceeded defined? [Clarity, Spec §FR-052]
- [ ] CHK013 Are the criteria for "partial success" of a batch operation defined? [Clarity, Spec §FR-059, data-model §11]

## Consistency

- [ ] CHK014 Are deletion methods consistent across US9, FR-043 and the CLI contract? [Consistency]
- [ ] CHK015 Are size metrics consistent across the specification, data model and report formats? [Consistency]
- [ ] CHK016 Are quarantine and Recycle Bin requirements consistent with free space constraints? [Consistency]
- [ ] CHK017 Are CLI and GUI requirements consistent with respect to feature parity? [Consistency, Spec §FR-074]
- [ ] CHK018 Are error codes and exit codes consistent between the CLI contract and functional requirements? [Consistency]

## Acceptance Criteria Quality

- [ ] CHK019 Are all success criteria measurable and do they contain numeric thresholds? [Measurability, Spec §Success Criteria]
- [ ] CHK020 Are Given/When/Then scenarios verifiable without knowledge of implementation details? [Measurability]
- [ ] CHK021 Are performance targets defined for all long-running operations, not just scanning? [Coverage, Gap]
- [ ] CHK022 Are verification conditions for the shredder specified (media type, confirmation method)? [Measurability, Spec §SC-006]

## Scenario Coverage

- [ ] CHK023 Are alternative flows covered (incremental scan, file system fallback, operation without permissions)? [Coverage]
- [ ] CHK024 Are exceptional situations covered (volume dismount, power failure, locked files)? [Coverage, Spec §Edge Cases]
- [ ] CHK025 Are recovery scenarios after operation interruption defined? [Recovery, Spec §Edge Cases]
- [ ] CHK026 Are scenarios of concurrent launch and conflicting tasks covered? [Coverage, Spec §Edge Cases]
- [ ] CHK027 Are scenarios for an empty result and no matches defined? [Coverage, Gap]

## Edge Cases

- [ ] CHK028 Are requirements defined for files larger than 4 GB and volumes larger than 8 TB? [Edge Case, Gap]
- [ ] CHK029 Are requirements defined for Unicode, emoji and very long names? [Edge Case, Spec §Edge Cases]
- [ ] CHK030 Are resource limits defined for reading archives (protection against archive bombs)? [Edge Case, Spec §FR-068]
- [ ] CHK031 Is the behavior defined for name conflicts when restoring from quarantine? [Edge Case, Spec §FR-044]

## Non-Functional Requirements

- [ ] CHK032 Are memory and time budgets specified for all stages (scanning, hashing, export)? [NFR, Gap]
- [ ] CHK033 Are monitoring overhead requirements defined? [NFR, Spec §SC-014]
- [ ] CHK034 Are privacy and offline operation requirements defined? [NFR, Spec §FR-094–FR-095]
- [ ] CHK035 Are journal reliability requirements under failures defined? [NFR, Spec §Edge Cases]
- [ ] CHK036 Are accessibility and keyboard navigation requirements for the interface specified? [Gap, NFR]

## Dependencies and Assumptions

- [ ] CHK037 Are external dependencies documented (RAR library, ETW permissions)? [Dependency, Spec §Assumptions]
- [ ] CHK038 Are the assumptions about administrator rights and local volumes validated? [Assumption]
- [ ] CHK039 Are format compatibility requirements for snapshots and reports defined? [Dependency, contracts]
- [ ] CHK040 Are the supported Windows versions and architecture specified? [Completeness, Spec §Assumptions]

## Ambiguities and Conflicts

- [ ] CHK041 Are there contradictions between safe deletion by default and the selected "ask every time" mode? [Conflict, Spec §Clarifications]
- [ ] CHK042 Is it clarified when the last access date is considered unreliable? [Ambiguity, Spec §FR-034]
- [ ] CHK043 Is physical size unambiguously interpreted for hard links from different directories? [Ambiguity, data-model §2]
- [ ] CHK044 Is the behavior defined when space is insufficient for the Recycle Bin and quarantine at the same time? [Ambiguity, Spec §FR-043]

## Traceability

- [ ] CHK045 Do all functional requirements have unique identifiers and links to stories? [Traceability]
- [ ] CHK046 Are success criteria linked to specific requirements and stories? [Traceability]
- [ ] CHK047 Are the decisions from the clarify stage reflected in all affected sections of the specification? [Traceability, Spec §Clarifications]

## Notes

- Mark `[x]` only after verifying the requirement quality criterion.
- Leave items unchecked if clarification or correction is required.
- `/speckit.implement` reads the checklist state as a gate and does not modify the markers.
- `checklists/requirements.md` has a separate lifecycle maintained by `/speckit.specify` and `/speckit.clarify`.
