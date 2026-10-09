# Player controls: manual device checks

Run on the target machine, on Wayland (KDE) and on X11. Mark each line pass or fail with the date.
Source: `plans/46-player-controls-ui/p1-ui-parity.md` section 19 (T-46a-056).

1. [ ] Single tap toggles chrome; the chrome hides 4 s after the last control click while playing; stays visible while paused.
2. [ ] Locking hides chrome; tapping reveals the unlock button; unlock restores chrome.
3. [ ] Double-click left and right seeks by 10 s and accumulates; reversing direction resets.
4. [ ] Horizontal drag shows the "+mm:ss [mm:ss]" text; release seeks to the target.
5. [ ] Vertical drag on the left changes brightness; at 0 further upward drag shows the veil (negative values).
6. [ ] Vertical drag on the right changes volume; with boost cap 30 and volume at max, continuing up shows "+N" and the mpv volume goes to 130.
7. [ ] Speed cycle 1.00 → 1.25 → … → 2.00 → 0.25.
8. [ ] Aspect cycle Fit → Stretch → Crop → Fit with the matching pill text.
9. [ ] Episode title opens the list; current episode is bold italic and scrolled into view; bookmark toggles persist.
10. [ ] Seekbar shows chapter ticks once each; drag shows thumbnail when the source provides tiles.
