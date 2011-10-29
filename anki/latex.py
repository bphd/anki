# -*- coding: utf-8 -*-
# Copyright: Damien Elmes <anki@ichi2.net>
# License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

import re, os, sys, shutil, cgi, subprocess
from anki.utils import checksum, call, namedtmp, tmpdir, isMac, stripHTML
from anki.hooks import addHook
from htmlentitydefs import entitydefs
from anki.lang import _

latexCmd = ["latex", "-interaction=nonstopmode"]
latexDviPngCmd = ["dvipng", "-D", "200", "-T", "tight"]
build = True # if off, use existing media but don't create new
regexps = {
    "standard": re.compile(r"\[latex\](.+?)\[/latex\]", re.DOTALL | re.IGNORECASE),
    "expression": re.compile(r"\[\$\](.+?)\[/\$\]", re.DOTALL | re.IGNORECASE),
    "math": re.compile(r"\[\$\$\](.+?)\[/\$\$\]", re.DOTALL | re.IGNORECASE),
    }

# add standard tex install location to osx
if isMac:
    os.environ['PATH'] += ":/usr/texbin"

def stripLatex(text):
    for match in regexps['standard'].finditer(text):
        text = text.replace(match.group(), "")
    for match in regexps['expression'].finditer(text):
        text = text.replace(match.group(), "")
    for match in regexps['math'].finditer(text):
        text = text.replace(match.group(), "")
    return text

def mungeQA(html, type, fields, model, data, deck):
    "Convert TEXT with embedded latex tags to image links."
    for match in regexps['standard'].finditer(html):
        html = html.replace(match.group(), _imgLink(deck, match.group(1), model))
    for match in regexps['expression'].finditer(html):
        html = html.replace(match.group(), _imgLink(
            deck, "$" + match.group(1) + "$", model))
    for match in regexps['math'].finditer(html):
        html = html.replace(match.group(), _imgLink(
            deck,
            "\\begin{displaymath}" + match.group(1) + "\\end{displaymath}", model))
    return html

def _imgLink(deck, latex, model):
    "Return an img link for LATEX, creating if necesssary."
    txt = _latexFromHtml(deck, latex)
    fname = "latex-%s.png" % checksum(txt.encode("utf8"))
    link = '<img src="%s">' % fname
    if os.path.exists(fname):
        return link
    elif not build:
        return u"[latex]%s[/latex]" % latex
    else:
        err = _buildImg(deck, txt, fname, model)
        if err:
            return err
        else:
            return link

def _latexFromHtml(deck, latex):
    "Convert entities and fix newlines."
    for match in re.compile("&([a-z]+);", re.IGNORECASE).finditer(latex):
        if match.group(1) in entitydefs:
            latex = latex.replace(match.group(), entitydefs[match.group(1)])
    latex = re.sub("<br( /)?>", "\n", latex)
    latex = stripHTML(latex)
    return latex

def _buildImg(deck, latex, fname, model):
    # add header/footer & convert to utf8
    latex = (model["latexPre"] + "\n" +
             latex + "\n" +
             model["latexPost"])
    latex = latex.encode("utf8")
    # it's only really secure if run in a jail, but these are the most common
    for bad in ("write18", "\\readline", "\\input", "\\include", "\\catcode",
                "\\openout", "\\write", "\\loop", "\\def", "\\shipout"):
        assert bad not in latex
    # write into a temp file
    log = open(namedtmp("latex_log.txt"), "w")
    texfile = file(namedtmp("tmp.tex"), "w")
    texfile.write(latex)
    texfile.close()
    mdir = deck.media.dir()
    oldcwd = os.getcwd()
    png = namedtmp("tmp.png")
    try:
        # generate dvi
        os.chdir(tmpdir())
        if call(latexCmd + ["tmp.tex"], stdout=log, stderr=log):
            return _errMsg("latex")
        # and png
        if call(latexDviPngCmd + ["tmp.dvi", "-o", "tmp.png"],
                stdout=log, stderr=log):
            return _errMsg("dvipng")
        # add to media
        shutil.copy2(png,
                     os.path.join(mdir, fname))
        return
    finally:
        os.chdir(oldcwd)

def _errMsg(type):
    msg = (_("Error executing %s.") % type) + "<br>"
    try:
        log = open(namedtmp("latex_log.txt", rm=False)).read()
        if not log:
            raise Exception()
        msg += "<small><pre>" + cgi.escape(log) + "</pre></small>"
    except:
        msg += _("Have you installed latex and dvipng?")
        pass
    return msg

# setup q/a filter
addHook("mungeQA", mungeQA)
