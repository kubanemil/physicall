# Research Codebase

Research the codebase to understand architecture, patterns, and implementation details related to: $ARGUMENTS

## Parsing Arguments

Arguments format: `[optional-reference] [topic-description]`

**Detecting a reference**: The first argument is a reference if it matches patterns like:

- Milestone: `m0.3`, `M1.2`
- Ticket: `PROJ-123`, `JIRA-456`
- Issue: `#42`, `GH-15`
- Any alphanumeric with numbers/dots/dashes

Examples:

- `/research m0.3 patient scheduling` → reference: `m0.3`, topic: `patient-scheduling`
- `/research KYO-123 gap closure` → reference: `kyo-123`, topic: `gap-closure`
- `/research twilio voice integration` → reference: none, topic: `twilio-voice-integration`
- `/research celery task patterns` → reference: none, topic: `celery-task-patterns`

## Context

First, read `AGENTS.md` to understand the project structure and conventions.

## Your Role

You are a **documentarian**, not an evaluator. Your job is to:

- Document the codebase AS-IS
- Provide factual observations with file paths and line numbers
- NOT suggest improvements, identify problems, or critique architecture

## Research Process

### Step 1: Initial File Reading

- Read `AGENTS.md` for project context
- Read ALL files mentioned by the user completely
- Do NOT spawn sub-tasks before reading these files yourself
- Understand the context before decomposing the research

### Step 2: Decompose Research Question

Break down the research into composable areas:

- Core implementation files
- Related utilities/helpers
- Configuration and setup
- Tests (if relevant)
- Dependencies and integrations

### Step 3: Parallel Research with Specialized Agents

Spawn parallel sub-agents using the Task tool for comprehensive coverage:

**File Discovery** - Use agent definition from `.claude/agents/codebase-locator.md`:

```
Find all files related to [topic]. Search for:
- Implementation files
- Test files
- Configuration
- Related modules
```

**Code Analysis** - Use agent definition from `.claude/agents/codebase-analyzer.md`:

```
Analyze how [component] works. Trace:
- Entry points and public APIs
- Data flow through the system
- Key functions and their purposes
```

**Pattern Discovery** - Use agent definition from `.claude/agents/codebase-pattern-finder.md`:

```
Find existing patterns related to [topic]. Look for:
- Similar implementations we can reference
- Conventions used in this area
- Test patterns for this type of code
```

**Documentation Search** - Use agent definition from `.claude/agents/docs-locator.md`:

```
Find existing documentation about [topic] in docs/ directory
```

### Step 4: Compile Findings

Wait for ALL agents to complete, then synthesize into a structured document.

### Step 5: Generate Research Document

Create a markdown file at: `docs/research/[filename].md`

**Filename format**:

- With reference: `[reference]-[topic-slug].md` (e.g., `m0.3-tenant-creation.md`, `proj-123-auth-system.md`)
- Without reference: `[topic-slug].md` (e.g., `event-sourcing-patterns.md`)

Rules:

- Convert reference to lowercase
- Use kebab-case for the topic slug

Use this structure:

```markdown
---
date: [ISO date]
researcher: [model name from session]
branch: [current git branch]
repository: [from AGENTS.md]
tags: [relevant, tags]
status: complete
---

# Research: [Topic]

## Research Question

[The original question or area being researched]

## Summary

[2-3 paragraph executive summary of findings]

## Detailed Findings

### [Area 1]

[Findings with specific file paths and line numbers]

### [Area 2]

[Findings with specific file paths and line numbers]

## Code References

- `backend/post_acute_care/models.py:123` - [Description]
- `backend/post_acute_care/views/patient_views.py:456` - [Description]
- `provider/src/app/patients/page.tsx:78` - [Description]

## Architecture Notes

[How the components fit together]

## Open Questions

[Any unresolved questions that need follow-up]
```

### Step 6: Sync and Present

- Write the research document to the file
- Present a summary to the user
- Ask if they have follow-up questions

## Critical Constraints

- **Document only**: No suggestions, improvements, or critiques
- **Be specific**: Include file paths and line numbers
- **Stay factual**: Report what IS, not what SHOULD BE
- **Be thorough**: Cover all relevant aspects of the question
- **Use agents**: Leverage specialized agents for parallel research

## Follow-up Questions

If the user asks follow-up questions, append findings to the SAME research document under a new section:

```markdown
## Follow-up: [Question]

[Additional findings]
```

## Handoff to Planning

**IMPORTANT**: At the end of EVERY response where you present research findings (initial or follow-up), you MUST include a "Next Steps" section:

```markdown
---

## Next Steps

If you're satisfied with this research, proceed to planning:

/plan [reference] [feature/task name] --research docs/research/[this-document].md

If you need more research:

- Ask follow-up questions and I'll append to this document
- Or request a new research topic with /research [reference] [new-topic]
```

Replace `[this-document].md` with the actual filename you created. Include the same reference (e.g., `m0.3`, `PROJ-123`) if one was used for this research, or omit if none.

Start by reading `AGENTS.md` and any files the user mentioned, then proceed with the research.
