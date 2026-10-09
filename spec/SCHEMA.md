# Kurobayomi UI spec schema

All spec files are JSON. Strings must be the real English text from
`i18n/src/commonMain/moko-resources/base/strings.xml` and
`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (resolve MR.strings.x / AYMR.strings.x).
Drop format placeholders like %1$s → use a plausible literal or keep "%s".

## Item (one row in a settings page, sheet or dialog)
```
{"type":"switch",   "title":str, "subtitle":str?, "default":bool}
{"type":"list",     "title":str, "subtitle":str?, "options":[str], "default":str}   // single choice
{"type":"multi",    "title":str, "subtitle":str?, "options":[str], "default":[str]}   // multi choice
{"type":"slider",   "title":str, "min":num, "max":num, "step":num, "default":num, "unit":str?}
{"type":"text",     "title":str, "subtitle":str?, "default":str?}                   // editable text
{"type":"tristate", "title":str}                                                    // filter: include/exclude/off
{"type":"sort",     "title":str, "options":[str]}                                   // sort picker w/ asc/desc
{"type":"chips",    "title":str, "options":[str], "default":str?}                   // segmented choice
{"type":"link",     "title":str, "subtitle":str?, "target":screen_id}               // navigates to another screen
{"type":"action",   "title":str, "subtitle":str?}                                   // button / opens dialog / does a thing
{"type":"info",     "title":str}                                                    // static text
```
## Group
`{"title":str|null, "items":[Item]}`

## Screen
```
{
  "id": str,                     // unique snake_case
  "title": str,
  "kind": "settings" | "custom",
  "toolbar": [{"label":str, "icon":str}],   // icon = freedesktop symbolic name guess, e.g. "system-search-symbolic"
  "menu": [str],                 // overflow menu entries
  "tabs": [str],                 // top tabs if any
  "groups": [Group],             // for settings-like screens
  "sheets": [{"id":str, "title":str, "tabs":[{"title":str, "groups":[Group]}]}],
  "dialogs": [{"id":str, "title":str, "text":str?, "groups":[Group], "buttons":[str]}],
  "empty": str?,                 // empty-state text
  "notes": str                   // precise layout description for custom screens: rows, badges, FABs, sections
}
```
