# M20 – Tool list instead of presets

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after the fix `fix/editor-launch-and-stash-diff` (findings #56, #57). Requested by the maintainer (2026-11-25, planning update 18). Last milestone before the final check.

## Goal

Preferences → External tools has no hard-coded editor and tool presets any more. The user builds a list of tools (name plus commands for opening, diffing and merging) and picks one entry for each purpose. A fresh installation has exactly one entry, VS Code, which can be edited, deleted and restored.

## Scope

In scope:

- A list of tool entries in Gitloom's settings, as many as the user wants
- One entry for the external editor, one for the diff tool and one for the merge tool, chosen from that list
- A "Manage tools…" sub-page to create, edit, duplicate and delete entries, and "Restore VS Code"
- The repository editor override, the command palette and the "No diff tool" dialog use the list
- Removal of `EditorPreset`, `ToolPreset` and `ToolPick` with their menus and messages
- Documentation, ADR-062, checklist section 7t

Out of scope:

- Detecting installed editors, importing tools from other applications
- A list for the terminal (the terminal command stays one field with auto-detection)
- Per-repository diff or merge tools
- Migrating settings of earlier versions (the maintainer is the only user; see decisions)
- A built-in check whether a command works (a failing start is reported by the launch toast, M19-03)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Source | Finding #56 (VS Code as external editor did not start) led to the maintainer's request: fewer predefined editors to avoid errors, VS Code stays, everything else is custom, any number of custom entries, the list is built by the user (2026-11-25). Predefined commands for tools that are not installed or not documented for a mode (Zed `--diff`, IntelliJ `diff`) were a source of errors. |
| Entry | `ToolEntry { id, name, open, diff, merge }`. `open`: editor command with `{path}` (appended when missing, as `launch::editor` does). `diff`: command with `{local}` and `{remote}`. `merge`: command with `{merged}`, optionally `{local}`, `{remote}`, `{base}`. An empty command means "not for this purpose". Commands are split into words like the terminal command (quotes, Windows keeps backslashes) and run exactly as before: the editor through `launch::editor`, diff and merge through `git difftool` / `git mergetool` with `-c …cmd=` (ADR-055). |
| Names and ids | The name is free text, unique (case-insensitive), non-empty. The id is stable and never shown: `vscode` for the built-in entry, `tool-<n>` (next free number) for the others. Renaming does not break a selection. |
| Default | A fresh installation has one entry: `vscode`, "VS Code": open `code {path}`, diff `code --wait --diff {local} {remote}`, merge `code --wait --merge {remote} {local} {base} {merged}` (the commands of the old presets, ADR-055). Nothing is pre-selected: editor None, diff and merge "Use Git config" (as today). |
| Stored list | `tools.entries` in `settings.json`. A missing key means the default list; a stored list, also an empty one, is kept as it is. Deleting VS Code therefore sticks. |
| Selection | `tools.editor`: the entry id or none. `tools.diff_tool` and `tools.merge_tool`: `ToolChoice` with `GitConfig`, `Tool(name)` (a Git tool), `None` and `Entry(id)` (replaces `Command`). A selection that points to a missing entry, or to an entry without the command for that purpose, is effectively "None"; the stored id stays, so filling the command in again makes it work. |
| Pickers | Editor: "None" and every entry with a valid `open` command. Diff and merge: "Use Git config (…)", "None", every entry with a valid command for that purpose, then every tool Git offers as "<name> (Git)" (all of them; no preset hides one any more; the internal `gitloom-custom` tool is never listed). Entries are listed in the order of the list. |
| Restore | "Restore VS Code" under the list: adds the `vscode` entry at the top if it is missing, resets its name and commands to the defaults if it exists. Other entries and the selections are not touched. Settings are not undo entries (ADR-043, ADR-055); the button is the way back (ADR-044). |
| Delete | Asks to confirm ("Delete <name>?", the text says where the entry is in use as editor, diff tool or merge tool). The selections that pointed to it become "None". |
| Validation | Shown under the field, never as a dialog. Name empty or taken; an unclosed quote; `diff` without `{local}` or without `{remote}`; `merge` without `{merged}`. An entry with a problem stays in the list (marked) and is not offered by the pickers for the purpose that has the problem. |
| No migration | The old fields (`tools.editor_command`, a `Command` selection for diff or merge) are dropped. A settings file from before M20 must still load without the "damaged settings" reset: unknown fields are ignored and an old `{"command": …}` selection reads as "Use Git config"; everything else in the file is kept. The user sets the tools again in the new list. |
| Repository override | `gitloom.externalEditor` stays a command text in the repository's Git config. Preferences → Repository → External editor offers "Use global setting" and the entries with an `open` command; choosing one stores that entry's command. A stored command that matches no entry is shown as "<program name> (not in the list)" and keeps working. The free text field is gone. |
| Palette | "Set: External editor ›" and "Repository: External editor ›" list the same entries; a new row "Manage tools…" in the Preferences search and the palette opens the sub-page. |
| "No diff tool" dialog | "Define tool…" opens Preferences → External tools (the pickers and "Manage tools…"). The text "No external diff tool is defined." stays. |
| Labels | "Waiting for <name>…", the toolbar button tooltip and the toasts use the entry's name; for a repository command without a matching entry, the program's file name. |
| Layout | Preferences → External tools: pickers for External editor, Diff tool, Merge tool (each with a "Manage tools…" link), Terminal command, "Delete .orig files after merging". "Manage tools…" is a sub-page like the themes (M18-05): title "Tools", "← External tools", Esc goes one level up. List on the left with a marker for problems, form on the right: Name, Open command, Diff command, Merge command, each with a one-line hint of its placeholders; buttons New tool, Duplicate, Delete, Restore VS Code. |
| Known limitation | Some tools return before the user closes them (a launcher that hands over to a running instance). `git difftool` then deletes its temporary files and the tool shows an empty or missing file. The user guide says so under troubleshooting; Gitloom does not detect it. |
| GitKraken | GitKraken offers a few editors and a custom path. The list is a deliberate Gitloom deviation (documented in UX-PARITY); no GitKraken check needed. |

## Maintainer answers and GitKraken verification items

Answered (2026-11-25): one shared list for editor, diff and merge; VS Code is the only predefined entry and can be restored; no migration (nobody else uses Gitloom yet, the maintainer wants to see the fresh-install case); Zed as a diff tool is not pursued separately (the preset disappears).

Open: none. For the maintainer's check: checklist section 7t.

## Milestone acceptance

- [ ] A fresh installation shows one entry "VS Code" under Manage tools; editor is None, diff and merge are "Use Git config".
- [ ] New tool, Duplicate and Delete work; names are unique; Delete asks first and resets the selections that used the entry.
- [ ] "Restore VS Code" brings back or resets the entry and leaves everything else alone; a deleted VS Code stays deleted after a restart.
- [ ] The editor, diff and merge pickers show exactly the entries that have a valid command for that purpose, plus Git's own choices for diff and merge.
- [ ] Alt+E, the toolbar button, the file menus, Ctrl+D and the merge tool use the chosen entries; "Waiting for <name>…" and the toasts show the entry's name.
- [ ] The repository override, the command palette and the "No diff tool" dialog use the list; the free text field of the repository editor is gone.
- [ ] A settings file from before M20 loads without the "damaged settings" message and keeps all other settings.
- [ ] `EditorPreset`, `ToolPreset` and `ToolPick` are gone; no preset text is left in the UI, the palette or the docs.
- [ ] ADR-062 is in `docs/DECISIONS.md`; user guide, UX-PARITY, release checklist (section 7t) and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M20-01 | Entries, selections, validation and loading in the settings | fix `fix/editor-launch-and-stash-diff` |
| M20-02 | External tools pickers and the "Manage tools" sub-page | M20-01 |
| M20-03 | Entry points: repository override, palette, dialog, labels; remove the presets | M20-01, M20-02 |
| M20-04 | Documentation and close | M20-01 … M20-03 |

---

## Tasks

### [x] M20-01 Entries, selections, validation and loading

- `crates/app/src/settings.rs`: `ToolEntry`, `ExternalTools.entries` (serde default: the VS Code entry; a stored empty list stays empty), `ExternalTools.editor: Option<String>`, `ToolChoice::Entry(String)` replacing `Command(String)`; the fields `editor_command` and the preset types are removed. Keep the settings `VERSION` (the change is tolerant, like ADR-055).
- Pure functions (no I/O): the effective editor command, the effective `ToolChoice` for diff and merge (missing entry or empty command → None), `entry_problems(&entries, index)` with the validation rules of the decision table, `next_id`, `unique_name` (for New tool and Duplicate: "New tool", "<name> copy", "<name> copy 2"), `restore_vscode`.
- Loading: an old file with `tools.editor_command` and with `"diff_tool": {"command": …}` or the same for `merge_tool` loads; the old selections read as described in the decision table; all other settings stay.
- The built-in commands live in one place (constants), taken from ADR-055.

Acceptance: unit tests: defaults of a fresh `Settings` (one entry, editor None, diff and merge GitConfig); a settings JSON without `entries` gets the default list, with `"entries": []` stays empty; round trip of entries and selections; a literal old settings file (with `editor_command` and a `command` selection) loads without the damaged-file path and keeps its other values; effective editor command for a selected, missing and empty-command entry; effective diff and merge choice; ids are unique and the `vscode` id is reserved for the built-in entry; names are unique case-insensitively; validation: empty name, taken name, unclosed quote, diff without `{local}` or `{remote}`, merge without `{merged}`; `unique_name` sequence; `restore_vscode` adds a missing entry at the top, resets a changed one, leaves other entries and selections as they are.

> Done (2026-11-25): `settings::ToolEntry`, `ExternalTools { entries, editor, … }` (`Default` holds the VS Code entry; `#[serde(default)]` takes it for a missing key, a stored list stays as it is), `ToolChoice::Entry`, old `{"command": …}` read through `ToolChoiceRepr` as "Use Git config" (any other unknown variant is still a damaged file), `editor_command` and other unknown keys ignored. Pure functions: `ExternalTools::editor_command`, `effective_choice`, `entry_command`, `tool_name`, `tool_options`, `editor_entries`; free `entry_problems`, `runnable`, `offered`, `next_id`, `unique_name`, `restore_vscode`; constants `VSCODE_*`. Deviation (ADR-034): removing `editor_command` and `ToolChoice::Command` leaves nothing for the old controls to write, so the External tools pickers (first bullet of M20-02), "Set: External editor ›" in the palette, "Define tool…" opening External tools, and the removal of `ToolPreset`, `ToolPick` and the editor/tool command fields (M20-03) moved into this task. `EditorPreset` stays until M20-03 for the repository override. Decisions: a name problem keeps an entry out of the pickers but does not stop a selected entry from running; a command problem makes that purpose "None" (like an empty command), so an unclosed quote now shows the "No diff tool" dialog instead of "Diff tool failed". "New tool" when taken continues as "New tool 2".

### [x] M20-02 Pickers and the Manage tools sub-page

- Preferences → External tools: pickers "External editor", "Diff tool", "Merge tool" built from the list by the rules of the decision table (Git's tools from the existing `--tool-help` loading, now without the preset filter); a "Manage tools…" link; terminal command and "Delete .orig files after merging" stay. The command field and preset dropdown for editor, diff and merge are removed.
- Sub-page "Manage tools…" with the layout of the decision table, built like the themes sub-page (M18-05: header, "← External tools", Esc one level up). Fields edit the entry directly and save like the other settings; validation text under the field; marker for entries with problems in the list. New tool selects the new entry and focuses the name. Delete opens a confirm dialog that names where the entry is in use. "Restore VS Code" as in the decision table.
- Settings search (`settings_registry.rs`): a row "Manage tools…" with a deep link; the rows for the editor and tool pickers keep working.

Acceptance: app tests: the pickers list exactly the expected entries in the expected order for each purpose (entries without a command or with a problem are missing); choosing an entry stores its id; New tool, Duplicate and Delete change the list and the selections as specified (Delete after confirming, Esc cancels the confirm); the name rule and the command rules show their message and keep the entry out of the picker; Restore VS Code in the three cases (missing, changed, unchanged); Esc leaves the sub-page one level up; a deleted VS Code stays deleted after saving and loading. Headless snapshot tests (`ui_check`, ADR-035) of the External tools page and of the sub-page in Dusk and Light: default list, a selected entry with a validation message. No file I/O on the UI thread beyond what settings saving does today.

> Done (2026-11-25): `Category::Tools` ("Tools", sidebar and parent External tools, Esc one level up); `Page::tool_selected`; messages `ToolSelected`, `ToolNew` (focuses `TOOL_NAME_INPUT`), `ToolDuplicate` (inserted below the original), `ToolDelete { confirmed }` (confirm text from `delete_tool_text`, selections reset by `ExternalTools::delete_entry`, the next entry is shown), `ToolRestoreVsCode` (also shows VS Code), `ToolNameInput`, `ToolCommandInput(Purpose, _)`; every change saves like the other settings. A "Manage tools…" link next to each picker; registry row "Manage tools…" (sub-page, like "Edit themes"); the External editor row moved first to match the page. Snapshots `ui_check_external_tools` and `ui_check_manage_tools` (Dusk, Light). The pickers themselves came with M20-01 (see its note).

### [x] M20-03 Entry points and removal of the presets

- Repository override (Preferences → Repository → External editor, `ui/settings.rs`): picker "Use global setting" plus the entries with an `open` command; choosing one stores its command through `set_repository_editor`; a stored command without a matching entry shows as "<program name> (not in the list)"; the custom text field is removed.
- Palette (`settings_registry.rs`): the choices of "Set: External editor ›" and "Repository: External editor ›" come from the list; "Manage tools…" opens the sub-page.
- `tool_missing_dialog` and `Message::DefineTool`: "Define tool…" opens Preferences → External tools.
- Labels: `editor_name` and `tool_name` return the entry's name when the command belongs to an entry, otherwise the program's file name; the preset mapping is removed. "Waiting for <name>…", the toolbar tooltip and the launch toasts use them.
- Remove `EditorPreset`, `ToolPreset`, `ToolPick`, `Message::EditorPresetChosen` and the code and tests that only served them; search the code base and the docs for the preset names to be sure nothing is left (`rg -i "preset"` in the tool and editor context).

Acceptance: app tests: Alt+E, the toolbar button, the file menus and the palette open the editor of the selected entry (the command comes from the entry; the copy path of commit files still arrives as `{path}`); Ctrl+D and the file menus run the diff command of the selected entry through Git (`-c difftool.gitloom-custom.cmd=…`), the merge tool the merge command; a selection that points to a deleted entry behaves like None ("No external editor set" warning with "Open Preferences" for the editor, the "No diff tool" dialog for diff); the repository override stores the entry's command and shows its name, an unknown stored command shows "(not in the list)" and still launches; the palette lists exactly the entries with an `open` command; the "No diff tool" dialog's "Define tool…" opens External tools. `rg` finds no `EditorPreset`, `ToolPreset` or `ToolPick`.

> Done (2026-11-25): repository override picker from `Choice::RepoEditor.options` ("Use global setting (…)" plus the entries with an open command), choice resolved by `app::repo_editor_choice`, a stored command without an entry shows `settings::repo_editor_label` "<program> (not in the list)" (plus a line "This repository: <command>") and still launches; text field, `RepoEditorInput`, `RepoEditorSave` and the page fields removed; a "Manage tools…" link on that page too. Palette choices of both editor rows come from the list. `settings::editor_name(entries, command)` and `program_name` replace the preset mapping; `App::editor_label` names the toolbar tooltip ("Open this repository in VS Code (Alt+E)") and the start failure toast ("Could not start VS Code", `launch_toast` takes the name); "Waiting for <name>…" uses `ExternalTools::tool_name`. `EditorPreset` removed; `rg` finds no `EditorPreset`, `ToolPreset` or `ToolPick` in `crates/`. "Define tool…" already opened External tools since M20-01.

### [ ] M20-04 Documentation and close

- `docs/DECISIONS.md`: ADR-062 "Tool list instead of presets" (context: finding #56 and the maintainer's request; the decision table; consequences: no migration, the old selections are dropped, settings files keep their version; ADR-055 stays valid for how commands run, its list of presets is superseded).
- `docs/user/settings.md`: External tools: the list, how to add a tool with examples for the three commands and their placeholders, Restore VS Code. `docs/user/troubleshooting.md`: a tool that returns before you close it (empty or missing temporary files) and what to do; "The program was not found" with the way to Manage tools.
- `docs/UX-PARITY.md`: rows "External editor" and "Open diff/merge tool": the list as a deliberate deviation (GitKraken: a few editors and a custom path), no GitKraken check needed; mention in the deviations table.
- `CHANGELOG.md` `[Unreleased]`: one paragraph for the tool list (what changed for the user, that earlier editor and tool choices have to be set again).
- `docs/RELEASE-CHECKLIST.md`: new section "7t. Tool list (M20) (Windows required, Linux checked in CI)" with the items below.
- `./scripts/check.sh` green.
- Then the steps of "After M20".

Checklist 7t items:

- [ ] Start with a fresh settings file (move `settings.json` away): Preferences → External tools shows the three pickers (editor None, diff and merge Use Git config); "Manage tools…" lists only "VS Code" with its three commands.
- [ ] Choose VS Code as editor: Alt+E, the toolbar button and "Open in external editor" in a file menu open VS Code. Choose it as diff tool: Ctrl+D opens the VS Code diff; as merge tool: a conflicted file opens in VS Code's merge editor.
- [ ] New tool "Zed" with open command `zed {path}`: it appears in the editor picker only; add a diff command and it appears in the diff picker too. Duplicate it: the copy is named "Zed copy".
- [ ] Delete a tool that is in use: the confirm names where it is used; afterwards that picker shows None and Alt+E says "No external editor set" with "Open Preferences".
- [ ] Delete "VS Code", restart Gitloom: it stays deleted. "Restore VS Code" brings it back at the top; change its open command, "Restore VS Code" resets it; other entries are untouched.
- [ ] Type a diff command without `{remote}`: a message appears under the field and the entry is missing from the diff picker; fix it and it returns.
- [ ] Preferences → Repository → External editor offers "Use global setting" and the entries; palette "Set: External editor ›" lists the same entries.
- [ ] An older `settings.json` (copy one from before M20 if you have it) opens without a "damaged settings" message; editor and tool selections are empty and can be set again.

## After M20

All planned milestones are done. Do not ask for planning. Instead:

1. `docs/ROADMAP.md`: M20 done (table and section); "Current milestone" → "Final check and release 1.0.0 (M6-13, M6-15; ADR-042)" with the checklist sections 7 … 7t; in the following paragraph "after M20".
2. `docs/HANDOVER.md`: current state (M20 with its ADR); the maintainer to-do "Final check before 1.0.0" lists all checklist sections 7 … 7t and the open GitKraken verification items (M20 has none).
3. `docs/PLANNING-HANDOVER.md`: milestone history line for M20 (version "1.0.0 (unreleased)"), section 8 "Next: final check by the maintainer, then 1.0.0".
4. Stop with the milestone report and the line "Bereit für den finalen Test (Checkliste Abschnitte 7 … 7t); 1.0.0 auf Zuruf."
