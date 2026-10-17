"""Wrap index.html in a skeleton like the Artifact publisher's, for local snapshots."""

from pathlib import Path

here = Path(__file__).parent
body = (here / "index.html").read_text()
skeleton = (
    "<!doctype html><html><head><meta charset=\"utf-8\">"
    "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">"
    "<style>:root{color-scheme:light;padding-top:env(safe-area-inset-top,0px);"
    "padding-bottom:env(safe-area-inset-bottom,0px)}body{margin:0;font:14px system-ui,sans-serif}"
    "img{max-width:100%}[hidden]{display:none!important}</style></head><body>"
)
(here / "preview.html").write_text(skeleton + body + "</body></html>")
print(here / "preview.html")
