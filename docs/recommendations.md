# Recommendations waiting for a decision

Open questions with the recommended answer. Remove an entry once it is decided.

## Search

1. **Typing opens search on every page** (since f8a14fa: the shared search bar captures keys on its page).
   A screen that uses plain letter keys as shortcuts (the Library grid may) now opens search instead.
   Recommended: keep type-to-search and move any plain-letter shortcuts to Ctrl+letter.
   Alternative: turn key capture off; the bar then opens only with Ctrl+F or the search button.

2. **The global search results page keeps its own search box** in the page body, next to the view-options button.
   Recommended: move it onto the shared search bar (`ui::search_row`) too, so every search in the app is the same.
