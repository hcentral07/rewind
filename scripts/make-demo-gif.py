"""Draws docs/demo.gif (the animation at the top of the README). Needs Pillow.
Run:  python scripts/make-demo-gif.py
Everything is drawn from code, so the GIF never goes stale: edit the scenes and re-run."""
from PIL import Image, ImageDraw, ImageFont
import os

W, H = 960, 540
BG, PANEL, PANEL2 = "#0f0f14", "#17171f", "#1d1d27"
LINE, TXT, MUTE = "#2e2e3a", "#f4f4f5", "#8b8b96"
ACC, RED, GRN, AMB = "#8b7cff", "#f87171", "#4ade80", "#f59e0b"

def font(size, mono=False, bold=False):
    base = "/usr/share/fonts/truetype/dejavu/"
    name = "DejaVuSansMono.ttf" if mono else ("DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf")
    for p in (base + name, name):
        try:
            return ImageFont.truetype(p, size)
        except OSError:
            continue
    return ImageFont.load_default()

F, FS, FB, FT, FM, FMS = font(15), font(12), font(15, bold=True), font(34, bold=True), font(14, mono=True), font(12, mono=True)
frames, durs = [], []

def new():
    im = Image.new("RGB", (W, H), BG)
    return im, ImageDraw.Draw(im)

def rr(d, box, fill, r=10, outline=None):
    d.rounded_rectangle(box, r, fill=fill, outline=outline)

def add(im, ms):
    frames.append(im)
    durs.append(ms)

def caption(d, text, sub=None):
    d.text((W // 2, 30), text, font=FB, fill=TXT, anchor="mm")
    if sub:
        d.text((W // 2, 54), sub, font=FS, fill=MUTE, anchor="mm")

def window(d, x, y, w, h, title):
    rr(d, (x, y, x + w, y + h), PANEL, 12, LINE)
    d.line((x, y + 34, x + w, y + 34), fill=LINE)
    for i, c in enumerate(("#ff5f56", "#ffbd2e", "#27c93f")):
        d.ellipse((x + 14 + i * 20, y + 12, x + 24 + i * 20, y + 22), fill=c)
    d.text((x + w // 2, y + 17), title, font=FS, fill=MUTE, anchor="mm")

def term(d, lines, x=80, y=90, w=800, h=400, title="terminal"):
    window(d, x, y, w, h, title)
    yy = y + 50
    for text, color in lines:
        d.text((x + 20, yy), text, font=FM, fill=color)
        yy += 22

# ---- scene 0: title -----------------------------------------------------------------
for k in range(2):
    im, d = new()
    d.text((W // 2, 200), "Rewind", font=font(64, bold=True), fill=TXT, anchor="mm")
    d.text((W // 2, 262), "undo for your files", font=font(26), fill=ACC, anchor="mm")
    d.text((W // 2, 330), "v0.3  \u00b7  AI agent undo  \u00b7  ransomware tripwire  \u00b7  plugins  \u00b7  Windows + Linux", font=F, fill=MUTE, anchor="mm")
    add(im, 1400 if k == 0 else 600)

# ---- scene 1: AI agent run, typed ---------------------------------------------------
cmd = "$ rewind run -- claude \"refactor the auth module\""
out = [
    ("[rewind] checkpoint saved, run 1 started in ~/code/shop", MUTE),
    ("", TXT),
    ("  ... the agent works for a few minutes ...", MUTE),
    ("", TXT),
    ("[rewind] run finished", TXT),
    ("run 1 (claude) in ~/code/shop", TXT),
    ("  14 created, 31 modified, 6 deleted  (+812 -1290 lines)", TXT),
    ("  ! deleted 6 files", AMB),
    ("  ! touched files that look like secrets: .env", AMB),
    ("  ! auth/session.rs lost 212 lines and gained 18", AMB),
]
for n in range(0, len(cmd) + 1, 4):
    im, d = new()
    caption(d, "1. Wrap any AI coding agent", "checkpoint first, full report after")
    term(d, [(cmd[:n] + ("_" if n < len(cmd) else ""), GRN)])
    add(im, 70)
for k in range(1, len(out) + 1):
    im, d = new()
    caption(d, "1. Wrap any AI coding agent", "checkpoint first, full report after")
    term(d, [(cmd, GRN)] + [(t, c) for t, c in out[:k]])
    add(im, 260 if k < 5 else 420)
add(im, 900)

cmd2 = "$ rewind agent undo 1 --apply --remove-new"
out2 = [("restored 31, recreated 6, removed 14", GRN), ("your project is back exactly as it was.", TXT)]
for n in range(0, len(cmd2) + 1, 5):
    im, d = new()
    caption(d, "2. Undo the whole run in one command", "a label is saved first, so even the undo can be undone")
    term(d, [(cmd, GRN)] + [(t, c) for t, c in out] + [(cmd2[:n], GRN)])
    add(im, 60)
for k in range(1, 3):
    im, d = new()
    caption(d, "2. Undo the whole run in one command", "a label is saved first, so even the undo can be undone")
    term(d, [(cmd, GRN)] + [(t, c) for t, c in out] + [(cmd2, GRN)] + out2[:k])
    add(im, 700)
add(im, 1200)

# ---- scene 2: tripwire --------------------------------------------------------------
def files_panel(d, encrypted):
    window(d, 80, 90, 800, 400, "Rewind \u00b7 ~/Documents")
    names = ["budget-2026.xlsx", "thesis-final.docx", "photos/trip-01.jpg", "taxes.pdf", "notes.md", "contract.docx", "resume.pdf"]
    for i, n in enumerate(names):
        y = 140 + i * 40
        rr(d, (100, y, 860, y + 32), PANEL2, 8)
        shown = n + (".locked" if encrypted else "")
        d.text((116, y + 16), shown, font=F, fill=RED if encrypted else TXT, anchor="lm")
        d.text((840, y + 16), "random bytes" if encrypted else "ok", font=FS, fill=RED if encrypted else MUTE, anchor="rm")

for k in range(2):
    im, d = new()
    caption(d, "3. Ransomware tripwire", "a burst of changes, or files turning into noise, trips the wire")
    files_panel(d, False)
    add(im, 700)
for k in range(3):
    im, d = new()
    caption(d, "3. Ransomware tripwire", "a burst of changes, or files turning into noise, trips the wire")
    files_panel(d, True)
    if k >= 1:
        rr(d, (250, 190, 710, 380), "#2a1215", 12, "#7f2a31")
        d.text((480, 222), "Many files changed at once", font=FB, fill=RED, anchor="mm")
        d.text((480, 262), "212 files changed within 4 seconds (9 now look encrypted).", font=FS, fill=TXT, anchor="mm")
        d.text((480, 284), "The last good version of each is pinned.", font=FS, fill=TXT, anchor="mm")
        rr(d, (330, 322, 630, 358), ACC, 8)
        d.text((480, 340), "Put the folder back", font=FB, fill="#0f0f14", anchor="mm")
    add(im, 900)
im, d = new()
caption(d, "3. Ransomware tripwire", "one click, everything back")
files_panel(d, False)
rr(d, (250, 436, 710, 480), "#10261a", 10, "#1f6b3d")
d.text((480, 458), "Put back 212 files. Nothing was lost.", font=FB, fill=GRN, anchor="mm")
add(im, 1800)

# ---- scene 3: right click -----------------------------------------------------------
def explorer(d, menu, panel):
    window(d, 80, 90, 800, 400, "Documents")
    for i, n in enumerate(["Projects", "budget-2026.xlsx", "thesis-final.docx", "notes.md"]):
        y = 140 + i * 44
        sel = (i == 1)
        rr(d, (100, y, 860, y + 36), "#262345" if sel else PANEL2, 8)
        d.text((118, y + 18), n, font=F, fill=TXT, anchor="lm")
    if menu:
        rr(d, (300, 210, 560, 330), "#22222c", 8, "#3a3a48")
        items = ["Open", "Copy", "Rename", "Delete"]
        for i, t in enumerate(items):
            d.text((320, 232 + i * 22), t, font=F, fill=MUTE, anchor="lm")
        rr(d, (306, 292, 554, 322), "#3a3590", 6)
        d.text((320, 307), "Rewind: show history", font=FB, fill=TXT, anchor="lm")

im, d = new(); caption(d, "4. Right-click any file or folder", "Windows, and Nautilus / Dolphin / Nemo on Linux"); explorer(d, False, False); add(im, 700)
im, d = new(); caption(d, "4. Right-click any file or folder", "Windows, and Nautilus / Dolphin / Nemo on Linux"); explorer(d, True, False); add(im, 1200)
im, d = new()
caption(d, "4. Right-click any file or folder", "opens its timeline in one click")
window(d, 80, 90, 800, 400, "Rewind \u00b7 budget-2026.xlsx")
for i, (v, t, s) in enumerate([("v7", "today 10:42", "Modified: +3 -1"), ("v6", "today 09:15", "Modified: +12 -4"), ("v5", "yesterday", "Modified: +1 -1"), ("v4", "Mon 16:20", "Created")]):
    y = 140 + i * 56
    rr(d, (100, y, 860, y + 46), PANEL2, 8, ACC if i == 0 else None)
    d.text((118, y + 23), v, font=FB, fill=ACC, anchor="lm")
    d.text((180, y + 23), t, font=F, fill=TXT, anchor="lm")
    d.text((420, y + 23), s, font=FS, fill=MUTE, anchor="lm")
    rr(d, (770, y + 9, 846, y + 37), "#2a2a36", 6)
    d.text((808, y + 23), "Restore", font=FS, fill=TXT, anchor="mm")
add(im, 2200)

# ---- scene 4: outro ------------------------------------------------------------------
im, d = new()
d.text((W // 2, 190), "Rewind 0.3", font=font(52, bold=True), fill=TXT, anchor="mm")
d.text((W // 2, 250), "Windows installer  \u00b7  Linux app + CLI  \u00b7  one engine", font=font(20), fill=GRN, anchor="mm")
d.text((W // 2, 300), "github.com/hcentral07/rewind/releases", font=font(20), fill=ACC, anchor="mm")
d.text((W // 2, 350), "free \u00b7 open source \u00b7 everything stays on your machine", font=F, fill=MUTE, anchor="mm")
add(im, 2600)

os.makedirs("docs", exist_ok=True)
pal = [f.convert("P", palette=Image.ADAPTIVE, colors=64) for f in frames]
pal[0].save("docs/demo.gif", save_all=True, append_images=pal[1:], duration=durs, loop=0, optimize=True, disposal=1)
print("docs/demo.gif", len(frames), "frames", os.path.getsize("docs/demo.gif") // 1024, "KB")
