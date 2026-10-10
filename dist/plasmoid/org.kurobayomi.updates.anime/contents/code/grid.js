.pragma library

// Grid cell pitch of the Android widget, in logical px (Aniyomi GlanceUtils: 64 x 95 per cell). The Android
// minWidth/minHeight/minResize*/maxResize* values give these sizes and are not ported as separate constants:
// minimum one cell (64 x 95), default 4 x 2 cells (256 x 190), maximum 10 x 10 cells (640 x 950).
var CELL_W = 64, CELL_H = 95, MAX_ROWS = 10, MAX_COLS = 10

function clamp(n, lo, hi) { return Math.min(hi, Math.max(lo, n)) }

// rows = clamp(trunc((height - padTop - padBottom) / CELL_H), 1, MAX_ROWS); cols = clamp(trunc(width / CELL_W), 1, MAX_COLS)
function rowsCols(width, height, padTop, padBottom) {
    var h = height - padTop - padBottom
    return {
        rows: clamp(Math.trunc(h / CELL_H), 1, MAX_ROWS),
        cols: clamp(Math.trunc(width / CELL_W), 1, MAX_COLS)
    }
}

// Splits items into rows of at most `cols` cells, so a short last row can be centred.
function chunk(items, cols) {
    var out = []
    for (var i = 0; i < items.length; i += cols)
        out.push(items.slice(i, i + cols))
    return out
}

// The command that feeds the grid.
function feedCommand(kind, rows, cols) {
    return "kurobayomi --updates-json --kind " + kind + " --rows " + rows + " --cols " + cols
}

// The command a cell runs: the deep-link form the app parses is `manga=<id>` / `anime=<id>`.
function openCommand(kind, entryId) {
    return "kurobayomi --open " + kind + "=" + Math.trunc(Number(entryId))
}
