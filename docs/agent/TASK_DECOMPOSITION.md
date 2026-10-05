# Task Decomposition Rules

## 1. Observable result
Every task must have a verifiable result via test, command, or artifact.

## 2. Single primary responsibility
Break down by behavior/responsibility, not by file.

## 3. One boundary at a time
Avoid mixing Core, hardware, USB, simulator, and configuration within a
single task. Integration between two layers can be a task, but the
layers must first be stable.

## 4. Separate risks
Separate protocol, memory, timing, concurrency, hardware, and dependencies.

## 5. Spike before integration
For an unknown technology:
1. spike;
2. verify version/license/compatibility;
3. compile a minimal example;
4. decision;
5. integration in a subsequent task.

## 6. Size
Split if, approximately:
- > 10 files;
- > 300 net lines;
- > 5 independent steps;
- > 2 architectural decisions.

These thresholds are heuristics and must be justified if exceeded.

## 7. Blockers
A blocked task becomes `BLOCKED` with reason, missing information,
options, and next action.

## 8. DONE requires proof
Compilation alone is not sufficient.

## Example ControlProcessor
1. CONTROL-001 values and invariants
2. CONTROL-002 clamp
3. CONTROL-003 normalization
4. CONTROL-004 inversion/dead-zone
5. CONTROL-005 curves
6. CONTROL-006 filter
7. CONTROL-007 hysteresis
8. CONTROL-008 composition
9. CONTROL-009 SensorEvent -> ControlEvent
10. CONTROL-010 integration tests

Each task must leave the repository compilable and testable.
