# Create Implementation Plan

Create a detailed implementation plan for: $ARGUMENTS

## Parsing Arguments

Arguments format: `[optional-reference] [feature-description] [--research path]`

**Detecting a reference**: The first argument is a reference if it matches patterns like:

- Milestone: `m0.3`, `M1.2`
- Ticket: `PROJ-123`, `JIRA-456`
- Issue: `#42`, `GH-15`
- Any alphanumeric with numbers/dots/dashes

**Optional flags**:

- `--research [path]`: Path to a research document to read first

Examples:

- `/plan m0.3 tenant creation` → reference: `m0.3`, feature: `tenant-creation`
- `/plan PROJ-123 user auth --research docs/research/proj-123-auth-system.md` → reference: `proj-123`, feature: `user-auth`
- `/plan add user roles` → reference: none, feature: `add-user-roles`

If `--research` is provided, read that document FIRST before any other context gathering.

## Context

First, read `AGENTS.md` to understand the project structure, conventions, and build commands.

## Your Role

You are a **technical architect** creating an executable specification. The plan must be:

- Precise enough that implementation requires no guesswork
- Phased with clear verification at each step
- Complete with all decisions made upfront
- Concise and focused on structure, contracts, and flow - NOT implementation code (target below 600 lines)

**Plan Should NOT Include**:

- Full function implementations
- Complete file contents
- Test file implementations
- Boilerplate code
- Code that will be written again during implementation

**Why**: The implemented code is the source of truth. Reviewing code twice (in plan, then in implementation) is redundant.

## Planning Process

### Step 1: Context Gathering

- Read `AGENTS.md` for project conventions
- Read ALL files provided by the user completely
- If a research document exists (via `--research`), read it first
- Spawn parallel exploration agents to understand:
  - Current codebase patterns and conventions
  - Existing similar implementations
  - Test patterns used in the project
- Present your understanding and ask clarifying questions

**Use these specialized agents** (definitions in `.claude/agents/`):

- `codebase-locator` - Find relevant files
- `codebase-analyzer` - Understand how existing code works
- `codebase-pattern-finder` - Find patterns to follow
- `thoughts-locator` - Find related documentation

### Step 2: Research & Discovery

- If the user corrects your understanding, DO NOT just accept it
- Spawn new research tasks to verify against actual code
- Track what you've researched vs. what remains unknown
- Present findings with multiple design options if applicable

### Step 3: Plan Structure

Before writing details, propose the phase structure:

- How many phases?
- What does each phase accomplish?
- What are the dependencies between phases?

Get user approval on structure before proceeding.

### Step 4: Detailed Plan Writing

Create a markdown file at: `docs/plans/[filename].md`

**Filename format**:

- With reference: `[reference]-[feature-slug].md` (e.g., `m0.3-tenant-creation.md`, `proj-123-user-auth.md`)
- Without reference: `[feature-slug].md` (e.g., `add-user-roles.md`)

Rules:

- Convert reference to lowercase
- Use kebab-case for the feature slug

Use this structure:

````markdown
---
date: [ISO date]
author: [model name from session]
status: draft | approved | in-progress | complete
branch: [target branch]
related_research: [path to research doc if exists]
---

# Plan: [Feature/Task Name]

## Overview

[1-2 paragraph summary of what this plan accomplishes]

## Architecture Decisions

[Bullet list of key decisions made and why]

- Decision 1: [choice] because [rationale]
- Decision 2: [choice] because [rationale]

## Prerequisites

- [ ] [Any setup or preparation needed]

---

## Phase 1: [Phase Name]

### Goal

[What this phase accomplishes]

### Files to Modify

- `backend/post_acute_care/models.py` - [What changes]
- `provider/src/app/patients/page.tsx` - [What changes]
- `backend/post_acute_care/views/new_view.py` - [New file: purpose]

### Module Design

**`ModuleName`** - [One sentence describing responsibility]

Key methods:

- `method_name(param1: Type, param2: Type) -> ReturnType` - [Purpose]
- `other_method(input: Input) -> Result | None` - [Purpose]

Model fields (if applicable):

```python
# Model fields only - no full implementation
class ModelName(models.Model):
    field1 = models.CharField(max_length=255)
    field2 = models.ForeignKey('OtherModel', on_delete=models.CASCADE)
```

**Frontend (TypeScript/React):**

Component/hook signature:

```typescript
// Types and signatures only - no implementation
interface ComponentProps {
  field1: string;
  field2: number;
}

function useHookName(param: Type): ReturnType;
```
````

### Integration

- [How this connects to existing code]
- [What calls what, data flow]

### Success Criteria

**Automated Verification:**
See `AGENTS.md` for exact commands. Run:

- Backend tests and lint for changes in `backend/`
- Frontend build and lint for changes in `provider/`, `meet/`, or `rounds-ai/`

**Manual Verification:**

- [ ] [Behavior to verify]
- [ ] [Expected output to check]

---

## Phase 2: [Phase Name]

[Same structure as Phase 1]

---

## Phase N: Final Verification

### Goal

Ensure all components work together end-to-end.

### Success Criteria

**Automated Verification:**

Run full project verification (see `AGENTS.md` → "Full Verification")

**Manual Verification:**

- [ ] [End-to-end test scenario]
- [ ] [Edge case to verify]

---

## Rollback Plan

[How to undo changes if something goes wrong - brief]

## Open Questions

[Any decisions deferred or questions for future consideration]

````

### Step 5: Review and Iterate

- Present the plan to the user
- Incorporate feedback
- Update the plan file
- Repeat until user approves

### Step 6: Finalize

- Update the plan status to `approved`
- Commit the plan: `git add docs/plans/[filename].md && git commit -m "[plan] [filename-without-extension]"`
  - Example with reference: `git commit -m "[plan] m0.3-tenant-creation"`
  - Example without: `git commit -m "[plan] add-user-roles"`

## Critical Constraints

- **No implementation code**: Describe what to build, not how to code it
- **No ambiguity**: Every decision must be made before finalizing
- **Specific file paths**: List exact files to create/modify
- **Testable criteria**: Each phase must have runnable verification
- **Atomic phases**: Each phase should be independently verifiable
- **Clear dependencies**: Note which phases depend on others
- **Reference AGENTS.md**: Don't duplicate build commands - reference `AGENTS.md`
- **Concise**: If your plan exceeds 600 lines, you're including too much detail

## Questions to Resolve Before Finalizing

Ensure the plan addresses:

1. What new files need to be created?
2. What existing files need modification?
3. What tests need to be written?
4. How do we verify each phase works?
5. How do components integrate?
6. What could go wrong and how do we handle it?

## Handoff to Implementation

**IMPORTANT**: At the end of EVERY response where you present a plan (draft or revised), you MUST include a "Next Steps" section:

```markdown
---

## Next Steps

If you approve this plan, proceed to implementation:

/implement docs/plans/[filename].md

If you want changes:

- Tell me what to revise and I'll update the plan
- Ask questions about specific phases or decisions
````

Replace `[filename].md` with the actual filename you created (e.g., `m0.3-tenant-creation.md` or `add-user-roles.md`).

When the user approves the plan:

1. Update the plan status to `approved` in the YAML frontmatter
2. Commit the plan file
3. Present the `/implement` command one final time

Start by gathering context and asking clarifying questions.
