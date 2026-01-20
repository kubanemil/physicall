# Implement Plan

Execute the implementation plan at: $ARGUMENTS

If no path provided, look for the most recent plan in `docs/plans/`.

## Context

First, read `AGENTS.md` to understand the project structure and build commands.

## Your Role

You are an **implementer** executing an approved plan. Your job is to:
- Follow the plan's intent while adapting to reality
- Verify each phase before proceeding
- Track progress with checkboxes in the plan file
- Pause for human verification at key points

## Implementation Process

### Step 1: Read and Understand
- Read `AGENTS.md` for build commands and conventions
- Read the plan file completely
- Check for any existing checkmarks (resume from where we left off)
- Read ALL files referenced in the plan (no pagination limits)
- Understand the current state vs. expected state

### Step 2: Execute Phase by Phase

For each phase:

1. **Announce**: "Starting Phase N: [Phase Name]"

2. **Implement**: Make the code changes specified in the plan
   - Follow the plan's intent, but adapt if reality differs
   - If the plan can't be followed exactly, STOP and explain why

3. **Run Automated Verification**:
   - Refer to `AGENTS.md` → "Build Commands" section for exact commands
   - Or use the `build-verifier` agent approach (see `.claude/agents/build-verifier.md`)
   - Run the verification commands specified in the plan's "Success Criteria"

4. **Update Plan Progress**: Check off completed items in the plan file

5. **Report Results**:
   ```
   ## Phase N Complete

   **Automated Verification:** (see AGENTS.md for commands)
   - [x] Backend tests: PASSED (X tests, 0 failures)
   - [x] Backend lint: PASSED
   - [x] Frontend build: PASSED
   - [x] Frontend lint: PASSED

   **Manual Verification Needed:**
   - [ ] [Item from plan]
   - [ ] [Item from plan]

   Please verify the manual items and confirm to proceed.
   ```

6. **PAUSE**: Wait for user confirmation before proceeding to next phase

### Step 3: Handle Problems

If you encounter issues:

1. **STOP** - Do not try to work around problems silently

2. **Analyze** - Think deeply about why the plan can't be followed

3. **Report clearly**:
   ```
   ## Problem Encountered in Phase N

   **Expected:** [What the plan said]
   **Actual:** [What we found]
   **Implication:** [Why this matters]

   **Options:**
   1. [Option A and tradeoffs]
   2. [Option B and tradeoffs]

   How would you like to proceed?
   ```

4. **Wait** for user decision before continuing

### Step 4: Track Progress

Update the plan file as you go:
- Check off completed prerequisite items: `- [x] Item`
- Check off automated verification: `- [x] Command: PASSED`
- Leave manual verification unchecked until user confirms

### Step 5: Final Verification

After all phases complete:

1. Run full project verification - see `AGENTS.md` → "Full Verification" section

2. Present summary of all changes made:
   ```markdown
   ## Implementation Complete

   ### Files Created
   - `backend/post_acute_care/services/new_service.py` - [Purpose]
   - `provider/src/app/feature/page.tsx` - [Purpose]

   ### Files Modified
   - `backend/post_acute_care/models.py` - [What changed]
   - `provider/src/lib/api.ts` - [What changed]

   ### Tests Added
   - `backend/post_acute_care/tests/test_new_feature.py` - [What's tested]

   ### Verification (see AGENTS.md for commands)
   - Backend: PASSED
   - Frontend: PASSED (if applicable)
   ```

3. Update plan status to `complete`

## Critical Constraints

- **Follow the plan**: The plan was carefully designed - trust it
- **Adapt when needed**: Reality can be messy; follow intent over letter
- **Pause for verification**: Never skip manual verification steps
- **Track everything**: Update checkboxes as you complete items
- **Communicate blockers**: Surface problems immediately, don't hide them
- **Use AGENTS.md**: Reference `AGENTS.md` for all build/test commands - don't hardcode

## Compaction Between Phases

For complex multi-phase work, after each phase is verified:
1. Update the plan file with current status
2. Note any deviations from the original plan
3. This creates a checkpoint we can resume from

## Resuming Implementation

If resuming from a previous session:
1. Read the plan file to see checked items
2. Find the first unchecked phase
3. Verify the previous phase's work is still intact
4. Continue from there

Start by reading `AGENTS.md` and the plan file, then announce which phase you'll begin with.
