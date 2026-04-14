# Work In Progress Documentation

This directory contains work-in-progress, progress tracking, fix summaries, and temporary documentation files.

## Purpose

These files are:
- **Progress tracking** documents created during development
- **Fix summaries** documenting specific bug fixes or improvements
- **Session summaries** from AI-assisted development sessions
- **Analysis documents** from troubleshooting or investigation
- **Completion markers** documenting when features were finished
- **Temporary notes** that may not be relevant long-term

## Guidelines

### What Belongs Here
- `*_COMPLETE.md` - Feature completion documents
- `*_FIX.md` - Bug fix summaries
- `*_SUMMARY.md` - Session or progress summaries
- `*_SUCCESS.md` - Success/completion markers
- `*_PROGRESS.md` - Progress tracking documents
- `*_IMPLEMENTATION.md` - Implementation details that are temporary
- Analysis and investigation documents
- Temporary notes and brainstorming

### What Should Be in docs/
- **Architecture guides** - System design and structure
- **How-to guides** - Instructions for common tasks
- **Reference documentation** - API, configuration, etc.
- **Stable documentation** - Won't change frequently
- **User-facing docs** - For contributors and users

## Moving Documents

When a WIP document becomes stable and valuable:
1. Review and clean up the content
2. Merge with existing docs or create a new stable doc
3. Move to the appropriate location in `docs/`
4. Update any references

## Cleanup

Periodically review this directory and:
- Archive very old documents
- Consolidate related summaries
- Delete obsolete fix documentation
- Move stable content to main docs

## See Also

- [docs/LOCAL_DEVELOPMENT.md](../LOCAL_DEVELOPMENT.md) - Main development guide
- [docs/ARCHITECTURE.md](../ARCHITECTURE.md) - System architecture
- [docs/TEST_DOCUMENTATION.md](../TEST_DOCUMENTATION.md) - Testing guide

