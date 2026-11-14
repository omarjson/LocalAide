---
name: Bug report
about: Something does not work as it should
title: "[BUG] "
labels: bug
assignees: ''
---

**What happened**
A short description of the problem.

**Steps to see it**
1. 
2. 
3. 

**What you expected**
What should have happened instead.

**Screenshots**
If they help.

**Your computer**
- System and version (for example Windows 11 23H2, macOS 15.1, Ubuntu 24.04):
- LocalAide version (Settings → About):
- Model in use, if any:

**Logs**
LocalAide keeps its logs in the `logs` folder of its data folder:

- Windows: `%USERPROFILE%\.aiassistant\logs\`
- macOS and Linux: `~/.aiassistant/logs/`

Attach `localaide.log`, and `llama-server.log` if the problem is with chat. Look through them first: they can contain file names from your computer.
