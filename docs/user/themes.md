# Themes

Gitloom has ten built-in color themes (Preferences → Appearance, see [Settings](settings.md)):

| Dark | Light |
|---|---|
| Dusk (default) | Light (default) |
| Studio Dark | Studio Light |
| Monokai | Sepia |
| Midnight | Daylight |
| Warm Dark | Warm Light |

Each brings its graph, Git flow and syntax colors. Dusk and Studio Dark use syntax themes of their own (also named Dusk and Studio Dark). You can also make your own theme: copy any theme, change its colors and use it like a built-in one.

Themes renamed in version 1.0 keep working from older settings: Dark is now Dusk, Dark+ Studio Dark, GitHub Dark Midnight, Light+ Studio Light, Solarized Light Sepia and GitHub Light Daylight. Gitloom writes the new names the next time it saves the settings.

### Syntax colors

Gitloom colors code by its grammar. Editors such as VS Code or Zed also color by meaning when a language server runs (semantic highlighting), so a few tokens can look different there, for example C# property and class names used in code. Gitloom has no language server; this is expected.

## Make a theme

1. Open **Preferences → Appearance → Manage themes…** (or "Edit themes…" in the command palette). The theme list opens in place of Appearance; **← Appearance** above the title or Esc goes back.
2. Select the theme to start from and click **New theme**. Gitloom saves a copy named "<name> copy" and opens it in the editor.
3. Change the name, choose whether it is a **Dark** or **Light** theme (this decides which System list offers it), and change the colors.
4. Click **Save**, or **Save and use** to switch to it at once.

Built-in themes are read only (lock icon): **New theme** copies them, **Export…** writes their file. Your own themes can also be renamed (**Rename…**, the file is renamed too) and deleted (**Delete…**, asks first). **Edit…** opens a theme of your own in the editor again.

## The editor

| Section | Colors |
|---|---|
| Main colors | Background, Surface, Chrome, Text, Muted text, Accent, Border, Success, Warning, Danger |
| More colors | Inset (framed lists), Hover, Selected, Header band, Backdrop (behind dialogs), Added and Removed characters (word diff) |
| Graph | The eight lane colors, used in turn |
| Git flow | Production, Hotfix, Release, Development, Unknown, and one to four feature colors ("Add feature color", "Remove"); with one feature color all feature branches use it |
| Syntax | The color scheme of code in diffs, blame, file history and the conflict view (the embedded themes) |

- Type a color as `#RRGGBB` (Backdrop and the two word diff colors also take `#RRGGBBAA` with transparency); the `#` may be left out. A value that is not a color is marked red and not used.
- Click a swatch for sliders (red, green, blue, and transparency where it applies).
- **Derive the other colors from the main colors** recomputes the More colors from the main colors. It never changes lanes, Git flow colors or the syntax theme. After you change a main color a hint offers it.
- The initials on graph nodes are black or white, whichever reads better on the lane colors.
- **Revert** goes back to the saved theme. Leaving the editor with changes asks "Discard changes?": **Close**, Esc, **← Appearance**, another Preferences page or closing Preferences.

## Preview and warnings

The **Preview** right of the colors shows sample graph rows, a diff and a panel with buttons in the edited colors. Only the preview uses them: the rest of the window, the editor included, keeps the theme in use, so no color choice can make Preferences unreadable.

Below the preview Gitloom lists contrast problems, for example "Text on background: 1.0:1 (4.5:1 needed)". They are hints: saving always works.

Checked: text on background, surface, chrome, hover and selected (4.5:1); muted text on background and surface (3:1); accent, success, warning and danger on background (3:1); the syntax theme's text on background (4.5:1).

## Import, export, files

- **Export…** saves the selected theme as a `.json` file anywhere.
- **Import…** copies a theme file into your themes folder after checking it. If a file of that name exists, choose **Replace** or **Keep both** (the import gets a numbered name). A file that is not a valid theme is not imported; Gitloom says why.
- **Open folder** opens the themes folder: `~/.config/gitloom/themes/` (Linux), `~/Library/Application Support/Gitloom/themes/` (macOS), `%APPDATA%\Gitloom\config\themes\` (Windows).
- A file in the folder that cannot be read stays untouched and is listed as "<file> (invalid: <reason>)".
- If the theme you use (or one chosen for System) is deleted or broken, Gitloom uses Dusk (or Light for the light System slot) and Preferences → Appearance says which theme was not found.

Themes are settings, not undo steps: the way back is **Revert**, **Delete…** or choosing another theme.

## File format

One JSON file per theme; the file name (without `.json`) identifies it.

| Key | Required | Content |
|---|---|---|
| `format` | no | `1` |
| `name` | yes | Name in the lists |
| `base` | yes | `"dark"` or `"light"` |
| `syntax` | no | An embedded syntax theme, e.g. `"Dusk"`, `"Studio Dark"`, `"OneHalfDark"`, `"GitHub"`, `"gruvbox-dark"` |
| `colors` | yes | `background`, `surface`, `chrome`, `text`, `text_muted`, `accent`, `border`, `success`, `warning`, `danger` (required); `inset`, `hover`, `selected`, `header_band`, `backdrop`, `diff_added_emphasis`, `diff_removed_emphasis` (optional) |
| `lanes` | no | Eight colors |
| `flow` | no | `production`, `hotfix`, `release`, `development`, `unknown`, `features` (one to four colors) |

Colors are `#RRGGBB` or `#RRGGBBAA`. Missing optional colors are derived from the main colors; missing `lanes`, `flow` and `syntax` come from the built-in Dusk or Light theme of the same base. Unknown keys are ignored. The editor always writes every key.

A minimal file written by hand:

```json
{
  "name": "Night shift",
  "base": "dark",
  "colors": {
    "background": "#1b1d23",
    "surface": "#23262d",
    "chrome": "#15171b",
    "text": "#e6e1d6",
    "text_muted": "#a39e93",
    "accent": "#e0a050",
    "border": "#33363d",
    "success": "#78b37a",
    "warning": "#e0c060",
    "danger": "#e07060"
  }
}
```
