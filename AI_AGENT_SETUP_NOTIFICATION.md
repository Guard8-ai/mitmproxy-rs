# 🚨 AI Agent Setup Required

**TaskGuard has been initialized with AI agent integration support.**

## Required Actions for AI Agents

### 1. Update Your Tool Instruction Files

Add the following to your AI tool instruction files:

#### For Claude Code (CLAUDE.md):
```markdown
## TaskGuard Integration

This project uses TaskGuard for AI-ready task management.

**Essential Reference**: See `AGENTIC_AI_TASKGUARD_GUIDE.md` for complete AI collaboration guide.

### Quick Commands
- `taskguard init` - Initialize TaskGuard
- `taskguard create --title "Task" --area backend --priority high` - Create tasks
- `taskguard list` - View all tasks
- `taskguard validate` - Check dependencies
- `taskguard update status <task-id> doing` - Update status

### Workflow
1. Use CLI commands for ALL task metadata updates
2. Create tasks across different areas (setup, backend, api, frontend, etc.)
3. Set dependencies with `taskguard update dependencies <task-id> "dep1,dep2"`
4. Validate frequently with `taskguard validate`
```

#### For Other AI Tools (GEMINI.md, etc.):
```markdown
## TaskGuard Integration

**Reference**: `AGENTIC_AI_TASKGUARD_GUIDE.md`

Use TaskGuard CLI commands for deterministic task management:
- Create: `taskguard create --title "Task" --area <area> --priority <priority>`
- Update: `taskguard update status <task-id> <status>`
- Dependencies: `taskguard update dependencies <task-id> "dep1,dep2"`
- Validate: `taskguard validate`
```

### 2. Key Integration Points

- **CLI-First**: Always use `taskguard update` commands instead of manual file editing
- **Area Distribution**: Spread tasks across multiple areas to avoid ID conflicts
- **Validation**: Run `taskguard validate` frequently to check dependencies
- **Granular Updates**: Use `taskguard task update <task-id> <item> done` for checklist items

### 3. Success Metrics

Your TaskGuard integration is working correctly when:
- ✅ Tasks are distributed across multiple areas
- ✅ Dependencies form clear chains
- ✅ `taskguard validate` shows no errors
- ✅ All metadata updates use CLI commands
- ✅ No template content remains in tasks

## Next Steps

1. **Read** `AGENTIC_AI_TASKGUARD_GUIDE.md` thoroughly
2. **Update** your tool instruction files with TaskGuard integration
3. **Test** the workflow with a few sample tasks
4. **Validate** your setup with `taskguard validate`

---

**This file can be deleted once you've completed the AI agent integration setup.**
