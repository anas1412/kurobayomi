"""Regenerates the archive test fixtures in this folder. Run: python3 -I make_fixtures.py

Every entry is deterministic so the tests can compare bytes: a magic prefix (JPEG or PNG) then
the pattern (i * 31) % 251. The RAR file is a hand-built RAR4 with stored entries, because no
`rar` binary is installed here; libarchive reads it. The 7z file needs the `7z` binary.
"""

import io
import os
import struct
import subprocess
import tarfile
import tempfile
import zipfile
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
JPEG = b"\xff\xd8\xff\xe0"
PNG = b"\x89PNG\r\n\x1a\n"


def entry(prefix, total):
    """`prefix`, then a deterministic pattern, `total` bytes in all."""
    return prefix + bytes((i * 31) % 251 for i in range(total - len(prefix)))


def rar4(entries):
    """Minimal RAR4 archive with stored (method 0x30) file entries."""
    out = bytearray(b"Rar!\x1a\x07\x00")

    def head(htype, flags, body):
        size = 7 + len(body)
        rest = struct.pack("<BHH", htype, flags, size) + body
        return struct.pack("<H", zlib.crc32(rest) & 0xFFFF) + rest

    out += head(0x73, 0x0000, bytes(6))  # MAIN_HEAD with reserved bytes
    for name, data in entries:
        raw = name.encode()
        body = struct.pack(
            "<IIBIIBBHI",
            len(data), len(data), 0, zlib.crc32(data) & 0xFFFFFFFF, 0, 29, 0x30, len(raw), 0x20,
        ) + raw
        out += head(0x74, 0x8000, body)
        out += data
    out += head(0x7B, 0x4000, b"")  # ENDARC
    return bytes(out)


CONTAINER_XML = """<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"""

PACKAGE_OPF = """<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
<manifest>
<item id="p2" href="text/p2.xhtml" media-type="application/xhtml+xml"/>
<item id="p1" href="text/p1.xhtml" media-type="application/xhtml+xml"/>
<item id="notes" href="text/notes.html" media-type="text/html"/>
</manifest>
<spine>
<itemref idref="p1"/>
<itemref idref="notes"/>
<itemref idref="p2"/>
</spine>
</package>"""

PAGE1_XHTML = """<html><body>
<img src="../images/a.jpg"/>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
<image xlink:href="../images/b.png"/>
</svg>
</body></html>"""

PAGE2_XHTML = """<html><body>
<img src="../images/a.jpg"/><img src="../images/c.png"/>
</body></html>"""


def write_epub(path):
    """Minimal EPUB: container, package with a text/html spine item, two XHTML pages. Page 1
    has an SVG `<image xlink:href>`; `a.jpg` is shared by both pages and listed once."""
    with zipfile.ZipFile(path, "w", zipfile.ZIP_STORED) as z:
        z.writestr("mimetype", "application/epub+zip")
        z.writestr("META-INF/container.xml", CONTAINER_XML)
        z.writestr("OEBPS/content.opf", PACKAGE_OPF)
        z.writestr("OEBPS/text/p1.xhtml", PAGE1_XHTML)
        z.writestr("OEBPS/text/p2.xhtml", PAGE2_XHTML)
        z.writestr("OEBPS/text/notes.html", "<html><body><img src='../images/x.jpg'/></body></html>")
        z.writestr("OEBPS/images/a.jpg", entry(JPEG, 64))
        z.writestr("OEBPS/images/b.png", entry(PNG, 64))
        z.writestr("OEBPS/images/c.png", entry(PNG, 80))


def main():
    jp = {
        "1.jpg": entry(JPEG, 300),
        "2.jpg": entry(JPEG, 120),
        "10.jpg": entry(JPEG, 50),
    }

    # Stored zip in archive order (not sorted), with an uppercase PNG, a text file and a directory.
    stored = os.path.join(HERE, "stored.zip")
    with zipfile.ZipFile(stored, "w", zipfile.ZIP_STORED) as z:
        z.writestr("10.jpg", jp["10.jpg"])
        z.writestr("2.jpg", jp["2.jpg"])
        z.writestr("1.jpg", jp["1.jpg"])
        z.writestr("002.PNG", entry(PNG, 100))
        z.writestr("notes.txt", b"notes\n")
        z.writestr(zipfile.ZipInfo("dir/"), b"")

    with open(stored, "rb") as src, open(os.path.join(HERE, "truncated.zip"), "wb") as f:
        f.write(src.read()[:100])

    # Deflate CBZ with one large entry so reads span many calls.
    with zipfile.ZipFile(os.path.join(HERE, "deflate.cbz"), "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("1.jpg", entry(JPEG, 20000))
        z.writestr("2.jpg", entry(JPEG, 96))
        z.writestr("notes.txt", b"notes\n")

    with open(os.path.join(HERE, "sample.cbr"), "wb") as f:
        f.write(rar4([("1.jpg", jp["1.jpg"]), ("2.jpg", jp["2.jpg"])]))

    for name, mode in (("sample.tar", "w"), ("sample.tar.gz", "w:gz")):
        with tarfile.open(os.path.join(HERE, name), mode) as t:
            for n in ("1.jpg", "2.jpg", "10.jpg"):
                info = tarfile.TarInfo(n)
                info.size = len(jp[n])
                t.addfile(info, io.BytesIO(jp[n]))

    with tarfile.open(os.path.join(HERE, "case.tar"), "w") as t:
        info = tarfile.TarInfo("a.jpg")
        info.size = len(entry(JPEG, 64))
        t.addfile(info, io.BytesIO(entry(JPEG, 64)))

    write_epub(os.path.join(HERE, "sample.epub"))

    # 7z packs files from a directory, so write the pages into a scratch folder first.
    with tempfile.TemporaryDirectory() as work:
        for n in ("1.jpg", "2.jpg", "10.jpg"):
            with open(os.path.join(work, n), "wb") as f:
                f.write(jp[n])
        dst = os.path.join(HERE, "sample.7z")
        if os.path.exists(dst):
            os.remove(dst)
        subprocess.run(
            ["7z", "a", "-t7z", "-bd", "-bb0", dst, "1.jpg", "2.jpg", "10.jpg"],
            cwd=work,
            check=True,
        )


if __name__ == "__main__":
    main()
