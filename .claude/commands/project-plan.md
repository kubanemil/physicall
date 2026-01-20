# Create Project Plan

Create a comprehensive project plan for: $ARGUMENTS

## Purpose

This command creates a **high-level project plan** - the strategic foundation document that defines vision, architecture, milestones, and technical decisions.

Works for:

- **Greenfield projects** - Starting from scratch
- **Project evolution** - Prototype → MVP → v2 → etc.
- **Major additions** - New service/module within existing product
- **Tech migrations** - Revisiting earlier decisions with new requirements

This is different from `/plan` which creates detailed implementation plans for specific features.

## Workflow Position

```
/project-plan  →  /prfaq (per feature)  →  /research  →  /plan  →  /implement
     ↑                    ↑
  Strategic          Press Release + FAQ
  planning           (Working Backwards)
```

## Parsing Arguments

Arguments: `[project-name-or-description] [--phase phase-name]`

**Optional flags**:

- `--phase [name]`: Indicates this is an evolution (e.g., `--phase mvp`, `--phase v2`)

Examples:

- `/project-plan multi-tenant workforce platform` - New project
- `/project-plan workforce platform --phase mvp` - Evolving from prototype to MVP
- `/project-plan add billing service` - Major addition to existing project
- `/project-plan` - Will prompt for details

## Your Role

You are a **product + system architect** creating a strategic project plan. Your job is to:

- Understand current state (if any exists)
- Understand the vision and constraints
- Ask clarifying questions - do NOT assume
- Present technical decisions as OPTIONS for the user to choose
- Research relevant frameworks/libraries that might inform decisions
- Build on existing foundations rather than reinventing

## Planning Process

### Step 0: Detect Project Context

**FIRST**, determine if this is greenfield or existing project.

Spawn a quick discovery agent to check:

```
Determine if this is an existing project or greenfield:
- Is there any source code? (look for: backend/, provider/, meet/, packages/, manage.py, package.json, etc.)
- Is there any documentation? (README, docs/, AGENTS.md, etc.)
- Is there an existing project plan or architecture document?
- Is there a Claude project configuration? or AGENTS.md file?
Return: project type (greenfield/existing), and if existing, list what was found.
```

Based on findings, classify as:

- **Greenfield**: No existing codebase or project plan → Skip to Step 2
- **Evolution**: Existing project plan found, moving to next phase → Do Step 1
- **Addition**: Existing codebase, adding major new capability → Do Step 1
- **Migration**: Existing codebase, changing technical decisions → Do Step 1

Tell the user what you detected and confirm before proceeding.

### Step 1: Research Current State (Existing Projects Only)

**IMPORTANT**: Use parallel agents to research thoroughly while keeping main context clean.

#### 1a. Spawn Parallel Research Agents

Use the Task tool to spawn these agents simultaneously:

**Agent 1: Project Structure & Conventions**

```
Discover the project structure and conventions:
- What is the directory structure? (where is source code, tests, docs, config?)
- Are there any convention/contributing guides?
- Is there a Claude project configuration? If so, read it for context.
- What documentation exists and where is it located?
Return a summary of project organization and any documented conventions.
```

**Agent 2: Architecture & Tech Stack**

```
Analyze the current architecture and tech stack:
- What programming language(s) and framework(s) are used?
- What is the overall system structure? (monolith, microservices, etc.)
- What are the main components and their responsibilities?
- What key libraries/dependencies are in use and why?
- What architectural patterns are evident? (MVC, CQRS, Event Sourcing, etc.)
- What databases or data stores are used?
Return a high-level architecture summary, not implementation details.
```

**Agent 3: Existing Plans & Decisions**

```
Find any existing planning documents or architectural decisions:
- Project plans, roadmaps, or vision documents
- Architecture Decision Records (ADRs)
- Technical specifications or design documents
- Research documents or technical notes
- Previous technical decisions and their rationale
Return a summary of key decisions and their status.
```

**Agent 4: Patterns & Conventions in Code**

```
Identify established patterns in the codebase:
- Code organization patterns
- Naming conventions
- Testing patterns and coverage
- Configuration and environment patterns
- Error handling patterns
Return patterns as a structured list with brief descriptions.
```

#### 1b. Synthesize Research Findings

Wait for ALL agents to complete, then compile into a Current State Summary:

```markdown
## Current State Summary

### Project Structure

[Where things are located - discovered, not assumed]

### Tech Stack

- **Backend**: Django + Django REST Framework (Python)
- **Frontend**: Next.js 14 with App Router (TypeScript)
- **Database**: PostgreSQL
- **Task Queue**: Celery with Redis
- **Key Libraries**: TanStack Query, Chakra UI/shadcn, LiveKit SDK, Twilio Voice SDK

### Architecture

[Brief description of current architecture based on findings]

### Established Patterns

- [Pattern 1]: [where/how used]
- [Pattern 2]: [where/how used]

### Existing Decisions

| Decision | Choice | Status                         |
| -------- | ------ | ------------------------------ |
| [what]   | [how]  | Keeping / Revisiting / Unknown |

### What's Working Well

- [observations from research]

### Potential Pain Points

- [observations from research]
```

Present this summary to user and **ask them to confirm or correct** before proceeding.

### Step 2: Gather Requirements

**For Greenfield**:

1. **Vision**: What problem does this solve? Who are the users?
2. **Core Features**: What are the 3-5 most important capabilities?
3. **Technical Constraints**: Any predetermined tech stack, hosting, or architectural decisions?
4. **Non-Goals**: What is explicitly out of scope?

**For Evolution** (prototype → MVP, v1 → v2):

1. **What worked well** in current phase?
2. **What needs to change** for next phase?
3. **New requirements** not in current scope?
4. **Technical debt** to address?
5. **Decisions to revisit** based on learnings?

**For Addition** (new service/module):

1. **What capability** is being added?
2. **How does it integrate** with existing system?
3. **Shared components** it will use?
4. **New components** it requires?
5. **Impact on existing** architecture?

**For Migration** (tech stack change):

1. **What's changing** and why?
2. **What's staying the same**?
3. **Migration path** - big bang or incremental?
4. **Compatibility requirements** during transition?

Ask these questions BEFORE producing any plan. Wait for answers.

### Step 3: Research Relevant Technologies

**For new tech decisions**, spawn a web research agent:

**Agent: Technology Research** (use `.claude/agents/web-researcher.md` pattern):

```
Research [framework/library] for [use case]:
- What does it provide out of the box?
- How does it handle [specific concern]?
- What are the common patterns?
- Any limitations or gotchas?
Return a summary of capabilities relevant to our decisions.
```

Run multiple research agents in parallel for different technologies.

**For existing tech**, research updates:

- Has the framework added features since project started?
- Are there better patterns now available?
- Any deprecations we need to handle?

Present findings and how they inform decisions.

### Step 4: Draft Architecture Overview

**For Greenfield**: Create new architecture diagram (ASCII)

**For Evolution/Addition**: Update existing diagram showing:

- What stays the same (existing components)
- What's new (NEW label)
- What changes (UPDATED label)

```
┌─────────────────────────────────────────────────────────────────┐
│                    System Architecture                          │
│                                                                 │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐     │
│  │  Existing    │     │   UPDATED    │     │    NEW       │     │
│  │  Component   │────▶│  Component   │────▶│  Component   │     │
│  └──────────────┘     └──────────────┘     └──────────────┘     │
│                                                                 │
│  Legend: [Existing] [UPDATED] [NEW]                             │
└─────────────────────────────────────────────────────────────────┘
```

Present and get feedback before proceeding.

### Step 5: Identify Technical Decisions

**Categorize decisions**:

1. **Decided (Keeping)**: Decisions from earlier phases that remain valid
2. **Decided (Changed)**: Earlier decisions being revised with rationale
3. **Open (New)**: New decisions needed for this phase
4. **Open (Revisiting)**: Earlier decisions being reconsidered

**CRITICAL**: Do NOT make decisions for the user. Present options and let them choose.

Format for open decisions:

```markdown
| ID  | Question         | Options                  | Framework Context    | Recommendation | Deadline  |
| --- | ---------------- | ------------------------ | -------------------- | -------------- | --------- |
| TD1 | How to handle X? | (A) ... <br>(B) ... <br> | Framework provides Y | Recommend (A)  | Before M1 |
```

### Step 6: Define Milestones

**For Greenfield**: Start from M0

**For Evolution**: Continue numbering from existing plan if one exists

```markdown
## Previous Milestones (if applicable)

| ID  | Milestone     | Status |
| --- | ------------- | ------ |
| M0  | Foundation    | DONE   |
| M1  | Core Features | DONE   |

## New Milestones

| ID  | Milestone            | Description     | Dependencies | Status      |
| --- | -------------------- | --------------- | ------------ | ----------- |
| M2  | Scale Infrastructure | Handle 10x load | M1           | NOT_STARTED |
```

### Step 7: Define Epics & User Stories

High-level stories only. Use proper user story format: **"As a [role], I can [action]"**

For Evolution, mark story types:

- **NEW**: Not in previous phase
- **ENHANCED**: Expanding on previous capability
- **MIGRATED**: Moving from old approach to new

```markdown
### M1: Core Features

#### E1.1: User Management

| ID     | Story                                       | Type | Acceptance Criteria       | Size |
| ------ | ------------------------------------------- | ---- | ------------------------- | ---- |
| E1.1.1 | As a user, I can register an account        | NEW  | Email verification works  | M    |
| E1.1.2 | As a user, I can reset my password          | NEW  | Reset link sent via email | S    |
| E1.1.3 | As an admin, I can deactivate user accounts | NEW  | User cannot log in after  | M    |
```

**Size guide**: S (hours), M (1-3 days), L (3-5 days), XL (> 5 days)

**IMPORTANT**: Stories describe WHAT, not HOW. No implementation details.

### Step 8: Identify Risks

Include risks specific to project type:

- **Evolution risks**: Migration complexity, backward compatibility, data migration
- **Addition risks**: Integration with existing system, performance impact
- **Greenfield risks**: Technology uncertainty, scope creep

### Step 9: Write the Plan Document

**Ask the user** where they want the project plan saved, or propose a location based on discovered project structure.

Common patterns (discover, don't assume):

- Root level: `PROJECT_PLAN.md`
- Docs folder: `docs/PROJECT_PLAN.md`
- Alongside existing planning docs if found

### Step 10: Review and Iterate

Present the plan section by section:

1. Current State Summary (if existing) - confirm accuracy
2. Vision & Goals - get approval
3. Architecture - get approval
4. Technical Decisions - get user choices on open questions
5. Milestones & Stories - get approval

Update the document after each round of feedback.

### Step 11: Finalize

When user approves:

1. Move chosen options from "Open" to "Decided" in Technical Decisions
2. Commit the plan with appropriate message

## Critical Constraints

- **Discover, don't assume**: Use agents to find project structure, don't hardcode paths
- **Use agents for research**: Don't pollute main context with codebase details
- **Research existing state first**: Don't ignore what's already built
- **No assumptions on decisions**: Present options, let user choose
- **High-level only**: No implementation details, file paths, or code
- **Scannable format**: Use tables, not prose paragraphs
- **Build on foundations**: Leverage existing patterns and decisions
- **Stories are WHAT not HOW**: Describe capabilities, not tasks

## What This Document is NOT

- NOT an implementation plan (use `/plan` for that)
- NOT a detailed technical specification
- NOT a timeline or schedule (no time estimates)
- NOT a feature-complete PRD (use `/prfaq` per feature)

## Handoff

At the end, present next steps using the workflow commands available in the project:

```markdown
---

## Next Steps

The project plan is complete. To start implementation:

1. **Pick a milestone** to start with
2. **For each epic/story**, use the workflow commands:
   - Feature analysis (if available)
   - Codebase research (if available)
   - Implementation planning (if available)
   - Implementation (if available)
```

## Commit Format

```bash
# Greenfield
git commit -m "[project-plan] Initial project plan for [project-name]"

# Evolution
git commit -m "[project-plan] [phase-name] phase plan for [project-name]"

# Addition
git commit -m "[project-plan] [feature-name] project plan"
```

Start by spawning a discovery agent to detect project context, then proceed accordingly.
