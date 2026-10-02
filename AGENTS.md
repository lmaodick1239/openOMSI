# Agent Guidelines: Subagent-Driven Development (SDD)

This repository adheres to a strict **Subagent-Driven Development (SDD)** workflow. Complex work, feature implementation, and iterative tasks are driven by a controller agent orchestrating fresh subagents for implementation and review.

---

<!-- ===================================================================== -->
<!-- USER-EDITABLE CONFIGURATION SECTION                                   -->
<!-- (Model names and fallbacks appear ONLY here. Edit this section only!) -->
<!-- ===================================================================== -->

## 1. Active Model Configuration (User-Editable)

> **Instructions for users / agents:**
> - Edit model identifiers and fallbacks **only in this section**.
> - **User input in chat overrides this section at all times**; models can be changed dynamically in real time.
> - Downstream workflow steps reference `models.code_writer.active` and `models.reviewer.active` from this block.

```yaml
models:
  code_writer:
    # Model used for code writing, TDD, task implementation, refactoring
    active: "customendpoint/Vvvvvip (2)/claude-opus-5"   # Alias: vvvvvip (2) opus-5
    fallbacks:
      - "customendpoint/Vvvvvip (5)/claude-opus-5"
      - "customendpoint/Duckcoding/claude-opus-5"

  reviewer:
    # Model used for task review, spec compliance, quality checks
    active: "customendpoint/Duckcoding/gpt-6.1-sol"      # Alias: duckcoding gpt-6.1-sol
    fallbacks:
      - "customendpoint/Duckcoding (2)/gpt-6.1-sol"
      - "customendpoint/Vvvvvip (6)/gpt-6.1-sol"
      - "customendpoint/Ciyuanapi/gpt-6.1-sol"
```

<!-- ===================================================================== -->
<!-- END OF USER-EDITABLE MODEL SECTION                                    -->
<!-- ===================================================================== -->

---

## 2. Core Directives & Authority Hierarchy

1. **User Input Overrides Everything**: User instructions provided in the chat at any time strictly supersede this document, default model configurations, task orders, and plan specifications.
2. **Dynamic Real-Time Model Updates**: The models used or requested are subject to change and can be updated in real time. Always check user input before dispatching.
3. **Fresh Agent Per Task, Fresh Agent Per Review**:
   - Each implementation task must be handled by a **new, dedicated implementer subagent** (never reuse dirty context across independent tasks).
   - Each completed task must be reviewed by a **new, dedicated reviewer subagent** before moving to the next task.
4. **Binary Overwrite Authorized**: You are **explicitly allowed to overwrite the executable** in `/mnt/More_Games/openOMSI-0.1.78-linux-x64/` (`openomsi`, `openomsi-launcher`) with newly built binaries.
5. **Continuous Execution**: Keep executing tasks in sequence autonomously without asking unnecessary "should I continue?" questions between tasks. Stop only when blocked by unresolvable errors, critical contradictions, or upon completing all tasks.

---

## 3. Subagent-Driven Development (SDD) Workflow

For each task in the active implementation plan, execute this closed-loop cycle:

```
┌────────────────────────────────────────────────────────┐
│ 1. Prepare Task Brief & Record BASE commit             │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────┐
│ 2. Dispatch Fresh Implementer Subagent                 │
│    • Model: models.code_writer.active (Section 1)      │
│    • Scope: TDD, implementation, unit tests, commit    │
│    • Output: Writes task report file                   │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────┐
│ 3. Generate Review Package (git diff / commit log)     │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────┐
│ 4. Dispatch Fresh Reviewer Subagent                    │
│    • Model: models.reviewer.active (Section 1)         │
│    • Dual-Gate Check: Spec Compliance ✅/❌ & Quality  │
└──────────────────────────┬─────────────────────────────┘
                           │
             ┌─────────────┴─────────────┐
             ▼                           ▼
     [Review Approved]           [Issues / Spec ❌]
             │                           │
             │                           ▼
             │                 [Fix Loop (Rounds 1–5)]
             │                 • Rounds 1–3: Resume implementer
             │                 • Rounds 4–5: Fresh implementer (or fallback model)
             │                 • Scoped re-review on fix diff
             │                           │
             └─────────────┬─────────────┘
                           ▼
┌────────────────────────────────────────────────────────┐
│ 5. Mark Complete in Progress Ledger                    │
└────────────────────────────────────────────────────────┘
```

### Step 1: Implementer Dispatch Contract
- **Harness Tool**: `task` with `agent_type: "general-purpose"`, `model: <models.code_writer.active from Section 1 (or user override)>`.
- **Implementer Context Requirements**:
  - Pass the exact task requirements, test commands, and target files.
  - Specify the report file location (e.g., `.superpowers/sdd/<plan-name>/task-N-report.md`).
  - Never dump previous full conversation history; provide only what the task touches.
- **Implementer Obligations**:
  - Follow Test-Driven Development (write/adjust tests first when applicable).
  - Implement minimal, surgical changes that satisfy the task.
  - Run focused tests (`cargo test -p <crate> <test_filter>`) and verify clean build.
  - Create a git commit with Conventional Commits format and the required Co-authored-by trailer:
    ```
    Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>
    ```
  - Write report to the assigned report file and return status (`DONE`, `DONE_WITH_CONCERNS`, `NEEDS_CONTEXT`, or `BLOCKED`).

### Step 2: Reviewer Dispatch Contract
- **Harness Tool**: `task` with `agent_type: "general-purpose"` (or `"code-review"`), `model: <models.reviewer.active from Section 1 (or user override)>`.
- **Reviewer Context Requirements**:
  - Task brief / specification.
  - Implementer's report file.
  - Exact diff package generated between pre-task `BASE` and post-task `HEAD` (`git diff BASE HEAD`).
- **Dual-Gate Review Rubric**:
  1. **Gate 1 - Spec Compliance (`Spec ✅` or `Spec ❌`)**:
     - Are all requested capabilities implemented?
     - Are any unrequested features or over-engineered abstractions added (YAGNI violation)?
  2. **Gate 2 - Code Quality (`Approved` or `Changes Required`)**:
     - Are there edge-case bugs, race conditions, memory leaks, or unhandled errors?
     - Are tests substantive rather than asserting trivialities?
     - Does the change conform to existing architecture and crate boundaries?

### Step 3: Fix Loop & Escalation Protocol
- If Gate 1 is `Spec ❌` or Gate 2 has `Critical` / `Important` findings:
  - **Rounds 1–3**: Resume the original implementer agent with the exact findings. Implementer fixes the issues, re-runs tests, and updates the report.
  - **Rounds 4–5**: If unresolved, dispatch a fresh implementer (optionally using a fallback model from Section 1, or higher-tier model requested by user) carrying the brief, report, and failed attempts history.
  - **Scoped Re-Review**: Run reviewer (using `models.reviewer.active`) only against the fix diff range (`FIX_BASE..HEAD`).
  - **Tripwire (Round 5)**: If still open after 5 rounds, stop and escalate/adjudicate in the progress ledger.
- Minor/cosmetic findings should be logged in the progress ledger as deferred and triaged during the final whole-branch review.

---

## 4. Build, Runtime & Content Environment

### Pre-Built Binary Location
- **Directory**: `/mnt/More_Games/openOMSI-0.1.78-linux-x64/`
- **Main Executable**: `/mnt/More_Games/openOMSI-0.1.78-linux-x64/openomsi`
- **Launcher Executable**: `/mnt/More_Games/openOMSI-0.1.78-linux-x64/openomsi-launcher`
- **Permission**: You are **ALLOWED to overwrite the executable** whenever deploying fresh builds for testing.

### Compilation & Overwrite Workflow
```bash
# 1. Targeted check & test during development:
cargo check -p omsi-app
cargo test -p omsi-launcher-core
cargo test -p omsi-app <test_name>

# 2. Compile release binaries:
cargo build --locked --release -p omsi-app -p omsi-launcher-core
# or run the repository build script:
sh scripts/build-linux.sh

# 3. Overwrite the executables in the runtime directory:
cp target/release/openomsi /mnt/More_Games/openOMSI-0.1.78-linux-x64/openomsi
cp target/release/openomsi-launcher /mnt/More_Games/openOMSI-0.1.78-linux-x64/openomsi-launcher
```

### Discovering Maps & Buses for Testing
The runtime directory at `/mnt/More_Games/openOMSI-0.1.78-linux-x64/` contains symlinks to OMSI 2 assets. You can `ls` these folders to find maps and buses for testing:

- **Finding Maps**:
  ```bash
  ls /mnt/More_Games/openOMSI-0.1.78-linux-x64/maps/
  ```
  *(Common test maps: `Grundorf`, `Berlin-Spandau`, `GreatGrundorf2`, `Grundorf Island 3.2`, etc.)*

- **Finding Buses / Vehicles**:
  ```bash
  ls /mnt/More_Games/openOMSI-0.1.78-linux-x64/Vehicles/
  ```
  *(Common test vehicles: `ADL_E200_KMB_AAS`, `(W6S1) Streetdeck_KMB`, `ADL_E400MMC_NWFB_3800`, `MAN_SD200`, etc.)*

- **Finding Scenery & Scripts**:
  ```bash
  ls /mnt/More_Games/openOMSI-0.1.78-linux-x64/Sceneryobjects/
  ls /mnt/More_Games/openOMSI-0.1.78-linux-x64/Scripts/
  ```

---

## 5. Progress Tracking & Ledger Standards

- All plan execution is tracked in a ledger file under `.superpowers/sdd/<plan-name>/progress.md`.
- Ledger entry format:
  ```markdown
  # SDD ledger — plan: <plan file path>
  Task 1: complete (commits abc1234..def5678, review clean)
  Task 2: fix round 1/5 (2 addressed, 0 open; commits ghi9012..jkl3456)
  Task 2: complete (commits def5678..jkl3456, review clean)
  ```
- After compaction or session interruptions, trust the ledger and `git log` over conversation recollection.
