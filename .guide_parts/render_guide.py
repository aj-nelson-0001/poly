#!/usr/bin/env python3
"""Render The_Poly_Language_Guide.md to PDF with reportlab.

Typography: a couple of points bigger throughout; inline code in headings
renders at the SAME size as the heading text, never smaller.
"""
import re
import sys

from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib import colors
from reportlab.pdfbase.pdfmetrics import registerFontFamily
from reportlab.pdfbase.pdfmetrics import stringWidth
from reportlab.pdfbase import pdfmetrics
from reportlab.platypus import (
    BaseDocTemplate, PageTemplate, Frame, Paragraph, Spacer, PageBreak,
    Table, TableStyle, XPreformatted, HRFlowable, KeepTogether,
)
from reportlab.platypus.tableofcontents import TableOfContents

# ---------------------------------------------------------------- fonts
pdfmetrics.registerFont(pdfmetrics.Font("PBody", "Helvetica", "WinAnsiEncoding"))
pdfmetrics.registerFont(pdfmetrics.Font("PBody-B", "Helvetica-Bold", "WinAnsiEncoding"))
pdfmetrics.registerFont(pdfmetrics.Font("PBody-I", "Helvetica-Oblique", "WinAnsiEncoding"))
pdfmetrics.registerFont(pdfmetrics.Font("PBody-BI", "Helvetica-BoldOblique", "WinAnsiEncoding"))
pdfmetrics.registerFont(pdfmetrics.Font("PMono", "Courier", "WinAnsiEncoding"))
pdfmetrics.registerFont(pdfmetrics.Font("PMono-B", "Courier-Bold", "WinAnsiEncoding"))
registerFontFamily(
    "PBody", normal="PBody", bold="PBody-B", italic="PBody-I", boldItalic="PBody-BI"
)
registerFontFamily(
    "PMono", normal="PMono", bold="PMono-B", italic="PMono", boldItalic="PMono-B"
)

# ---------------------------------------------------------------- sizes
PAGE_W, PAGE_H = A4
MARGIN = 50
USABLE = PAGE_W - 2 * MARGIN          # 495 pt

BODY = 11.5
BODY_LEAD = 16.0
H1 = 22.0
H2 = 16.0
H3 = 13.5
CODE = 10.0
CODE_LEAD = 13.6
TBL = 10.5
MONO_W = 0.60                          # Courier advance per point of size

TINT = {
    "poly": ("#EAF2FB", "#B8CFEA"),
    "rust": ("#FBF1E6", "#E4C9A8"),
    "bash": ("#F0F0F0", "#CCCCCC"),
    "toml": ("#F4F4F4", "#DDDDDD"),
    "text": ("#F4F4F4", "#DDDDDD"),
}
DEFAULT_TINT = ("#F4F4F4", "#DDDDDD")

# ---------------------------------------------------------------- styles
def st(name, **kw):
    base = dict(fontName="PBody", fontSize=BODY, leading=BODY_LEAD,
                textColor=colors.HexColor("#1A1A1A"), spaceBefore=0,
                spaceAfter=BODY * 0.55)
    base.update(kw)
    return ParagraphStyle(name, **base)

S = {
    "body":   st("body"),
    "h1":     st("h1", fontName="PBody-B", fontSize=H1, leading=H1 * 1.25,
                 spaceBefore=6, spaceAfter=12, keepWithNext=1),
    "h2":     st("h2", fontName="PBody-B", fontSize=H2, leading=H2 * 1.3,
                 spaceBefore=14, spaceAfter=8, keepWithNext=1),
    "h3":     st("h3", fontName="PBody-B", fontSize=H3, leading=H3 * 1.3,
                 spaceBefore=12, spaceAfter=6, keepWithNext=1),
    "bullet": st("bullet", leftIndent=16, bulletIndent=4, spaceAfter=BODY * 0.35),
    "quote":  st("quote", leftIndent=14, fontName="PBody-I",
                 textColor=colors.HexColor("#333333")),
    "title":  st("title", fontName="PBody-B", fontSize=34, leading=42,
                 spaceAfter=14),
    "sub":    st("sub", fontSize=13, leading=19,
                 textColor=colors.HexColor("#444444")),
    "toc_h":  st("toc_h", fontName="PBody-B", fontSize=H1, leading=H1 * 1.25,
                 spaceAfter=16),
    "label":  st("label", fontName="PBody-B", spaceBefore=4, spaceAfter=4,
                 keepWithNext=1),
    "cell":   st("cell", fontSize=TBL, leading=TBL * 1.35, spaceAfter=0),
    "cellh":  st("cellh", fontName="PBody-B", fontSize=TBL, leading=TBL * 1.35,
                 spaceAfter=0),
    "foot":   st("foot", fontSize=9.5, spaceAfter=0),
}

TOC_LEVELS = [
    st("toc0", fontName="PBody-B", fontSize=12, leading=17, spaceAfter=0,
       spaceBefore=6),
    st("toc1", fontSize=10.5, leading=15, leftIndent=14, spaceAfter=0,
       spaceBefore=0),
]

# ---------------------------------------------------------------- inline md
def esc(t):
    return t.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

def inline(text, mono_face="PMono"):
    """Markdown inline -> reportlab markup. Code spans keep the CURRENT size
    (no size attribute), so headings render code at full heading size."""
    parts = re.split(r"(`[^`]*`)", text)
    out = []
    for part in parts:
        if part.startswith("`") and part.endswith("`") and len(part) > 2:
            out.append('<font face="%s">%s</font>' % (mono_face, esc(part[1:-1])))
        else:
            t = esc(part)
            t = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", t)
            t = re.sub(r"(?<!\*)\*([^*\n]+?)\*(?!\*)", r"<i>\1</i>", t)
            out.append(t)
    return "".join(out)

# ---------------------------------------------------------------- code block
def code_block(lang, lines):
    lang = lang.split()[0] if lang else "text"
    tint, border = TINT.get(lang, DEFAULT_TINT)
    mono = "PMono"
    if lang in ("poly", "rust", "bash", "js", "c"):
        mono = "PMono"
    size = CODE
    longest = max((stringWidth(l, "PMono", size) for l in lines), default=0)
    if lines and longest > USABLE - 24:
        size = max(8.6, (USABLE - 24) / (longest / CODE))
    style = ParagraphStyle(
        "code", fontName=mono, fontSize=size, leading=size * 1.36,
        textColor=colors.HexColor("#1A1A2E"))
    # One row per line so long blocks can split across pages.
    rows = [[XPreformatted(esc(l) or " ", style)] for l in lines] or [[" "]]
    n = len(rows)
    t = Table(rows, colWidths=[USABLE])
    cmds = [
        ("BACKGROUND", (0, 0), (-1, -1), colors.HexColor(tint)),
        ("LINEBEFORE", (0, 0), (0, -1), 0.7, colors.HexColor(border)),
        ("LINEAFTER", (-1, 0), (-1, -1), 0.7, colors.HexColor(border)),
        ("LEFTPADDING", (0, 0), (-1, -1), 10),
        ("RIGHTPADDING", (0, 0), (-1, -1), 10),
        ("TOPPADDING", (0, 0), (-1, -1), 0),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 0),
        ("TOPPADDING", (0, 0), (-1, 0), 7),
        ("BOTTOMPADDING", (0, -1), (-1, -1), 7),
    ]
    if n == 1:
        cmds.append(("BOX", (0, 0), (-1, -1), 0.7, colors.HexColor(border)))
    else:
        cmds.append(("LINEABOVE", (0, 0), (-1, 0), 0.7, colors.HexColor(border)))
        cmds.append(("LINEBELOW", (0, -1), (-1, -1), 0.7, colors.HexColor(border)))
    t.setStyle(TableStyle(cmds))
    return [Spacer(1, 3), t, Spacer(1, 5)]

# ---------------------------------------------------------------- table
def md_table(rows):
    header = rows[0]
    body = [r for r in rows[1:] if not re.match(r"^\s*:?-{3,}", r[0].strip())]
    ncol = len(header)
    if len(body) > 1 and all(re.match(r"^\s*:?-{3,}", c.strip()) for c in rows[1]):
        body = rows[2:]
    widths = []
    for c in range(ncol):
        w = max([stringWidth(esc(r[c] if c < len(r) else ""), "PBody", TBL)
                 for r in rows if c < len(r)] + [30])
        widths.append(w)
    total = sum(widths)
    col_w = [max(60, w / total * USABLE) for w in widths]
    scale = USABLE / sum(col_w)
    col_w = [w * scale for w in col_w]

    data = []
    data.append([Paragraph(inline(c), S["cellh"]) for c in header])
    for r in body:
        data.append([Paragraph(inline(r[c] if c < len(r) else ""), S["cell"])
                     for c in range(ncol)])
    t = Table(data, colWidths=col_w, repeatRows=1)
    t.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#E8EEF6")),
        ("GRID", (0, 0), (-1, -1), 0.5, colors.HexColor("#B9C4D1")),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 4),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
        ("ROWBACKGROUNDS", (0, 1), (-1, -1),
         [colors.white, colors.HexColor("#F7F9FB")]),
    ]))
    return [Spacer(1, 4), t, Spacer(1, 6)]

# ---------------------------------------------------------------- doc
class GuideDoc(BaseDocTemplate):
    def beforeDocument(self):
        # Called at the start of every multiBuild pass, so bookmark keys
        # stay identical between passes and the TOC can converge.
        self.heading_seq = 0

    def afterFlowable(self, flowable):
        if isinstance(flowable, Paragraph):
            style = flowable.style.name
            if style in ("h1", "h2"):
                text = re.sub(r"<[^>]+>", "", flowable.text)
                text = text.replace("&amp;", "&").replace("&lt;", "<") \
                           .replace("&gt;", ">")
                level = 0 if style == "h1" else 1
                key = "h-%d" % self.heading_seq
                self.heading_seq += 1
                self.canv.bookmarkPage(key)
                self.canv.addOutlineEntry(text, key, level, level == 0)
                self.notify("TOCEntry", (level, text, self.page, key))

def footer(canvas, doc):
    if canvas.getPageNumber() < 2:
        return
    canvas.saveState()
    canvas.setFont("PBody", 9.5)
    canvas.setFillColor(colors.HexColor("#666666"))
    canvas.drawCentredString(PAGE_W / 2, 30, "page %d" % canvas.getPageNumber())
    canvas.restoreState()

# ---------------------------------------------------------------- parse
def parse(md):
    story = []
    lines = md.splitlines()
    i = 0
    first_h1_seen = False
    prev_was_break = False
    while i < len(lines):
        line = lines[i]

        fence = re.match(r"^~~~(\S*)\s*(.*)$", line)
        if fence:
            lang = fence.group(1)
            i += 1
            block = []
            while i < len(lines) and lines[i].strip() != "~~~":
                block.append(lines[i])
                i += 1
            i += 1
            story.extend(code_block(lang, block))
            prev_was_break = False
            continue

        if re.match(r"^#{1,3}\s", line):
            level = len(line) - len(line.lstrip("#"))
            text = line.lstrip("#").strip()
            if level == 1:
                if first_h1_seen and not prev_was_break:
                    story.append(PageBreak())
                story.append(Paragraph(inline(text, "PMono-B"), S["h1"]))
                first_h1_seen = True
            elif level == 2:
                story.append(Paragraph(inline(text, "PMono-B"), S["h2"]))
            else:
                story.append(Paragraph(inline(text, "PMono-B"), S["h3"]))
            prev_was_break = False
            i += 1
            continue

        if line.strip() == "---":
            story.append(Spacer(1, 4))
            story.append(HRFlowable(width="100%", thickness=0.7,
                                    color=colors.HexColor("#C9C9C9")))
            story.append(Spacer(1, 4))
            prev_was_break = False
            i += 1
            continue

        if line.startswith("|"):
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                cells = [c.strip() for c in lines[i].strip().strip("|").split("|")]
                rows.append(cells)
                i += 1
            story.extend(md_table(rows))
            prev_was_break = False
            continue

        if line.startswith(">"):
            quote = []
            while i < len(lines) and lines[i].startswith(">"):
                quote.append(lines[i].lstrip(">").strip())
                i += 1
            story.append(Paragraph(inline(" ".join(quote)), S["quote"]))
            prev_was_break = False
            continue

        if re.match(r"^\s*[-*]\s", line) or re.match(r"^\s*\d+\.\s", line):
            while i < len(lines) and (
                re.match(r"^\s*[-*]\s", lines[i]) or
                re.match(r"^\s*\d+\.\s", lines[i]) or
                (lines[i].startswith("  ") and lines[i].strip())
            ):
                m = re.match(r"^(\s*)[-*]\s+(.*)$", lines[i])
                num = re.match(r"^(\s*)(\d+\.)\s+(.*)$", lines[i])
                if m:
                    bullet = "\u2022"
                    content = m.group(2)
                elif num:
                    bullet = num.group(2)
                    content = num.group(3)
                else:
                    if story and isinstance(story[-1], Paragraph):
                        story[-1] = Paragraph(
                            story[-1].text + " " + inline(lines[i].strip()),
                            story[-1].style)
                    i += 1
                    continue
                story.append(Paragraph(inline(content), S["bullet"],
                                       bulletText=bullet))
                i += 1
            prev_was_break = False
            continue

        if not line.strip():
            i += 1
            continue

        para = [line]
        i += 1
        while (i < len(lines) and lines[i].strip() and
               not re.match(r"^(~~~|#{1,3}\s|\||>|\s*[-*]\s|\s*\d+\.\s|---$)",
                            lines[i])):
            para.append(lines[i])
            i += 1
        story.append(Paragraph(inline(" ".join(para)), S["body"]))
        prev_was_break = False
    return story

# ---------------------------------------------------------------- build
def build():
    md = open("The_Poly_Language_Guide.md", encoding="utf-8").read()

    # Title page
    story = [
        Spacer(1, 150),
        Paragraph("The Poly Language Guide", S["title"]),
        Paragraph("A friendly, step-by-step handbook that teaches you every "
                  "Poly command, with a Rust translation for each one.",
                  S["sub"]),
        Spacer(1, 40),
        Paragraph("Every example in Poly <b>and</b> in Rust. Written so a kid "
                  "can read it.", S["sub"]),
        Spacer(1, 170),
        Paragraph("October 2026", S["sub"]),
        PageBreak(),
    ]

    # Contents
    story.append(Paragraph("Contents", S["toc_h"]))
    toc = TableOfContents()
    toc.levelStyles = TOC_LEVELS
    toc.dotsMinLevel = 0
    story.append(toc)
    story.append(PageBreak())

    story.extend(parse(md))

    doc = GuideDoc(
        "The_Poly_Language_Guide.pdf", pagesize=A4,
        leftMargin=MARGIN, rightMargin=MARGIN,
        topMargin=52, bottomMargin=52,
        title="The Poly Language Guide",
        author="The Poly Book",
    )
    doc.frame = None
    frame = Frame(MARGIN, 52, USABLE, PAGE_H - 52 - 52, id="main")
    doc.addPageTemplates([PageTemplate(id="page", frames=[frame],
                                       onPage=footer)])
    doc.multiBuild(story)
    print("OK: The_Poly_Language_Guide.pdf")

if __name__ == "__main__":
    build()
