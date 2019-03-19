#!/usr/bin/env python3.7

import re

vernacular_binder = [
    "Definition",
    "Inductive",
    "Fixpoint",
    "Theorem",
    "Function",
    "Remark",
    "Hypothesis",
    "Lemma",
    "Example",
    "Ltac",
    "Record",
    "Variable",
    "Section",
    "End",
    "Instance",
    "Module",
    "Context"
]
vernacular_words = vernacular_binder + [
    "Proof",
    "Qed",
    "Defined",
    "Require",
    "Import",
    "Export",
    "Print",
    "Assumptions",
    "Local",
    "Open",
    "Scope",
    "Admitted",
    "Notation",
    "Set",
    "Unset",
    "Implicit",
]

local_binder = [
    "forall",
    "fun"
]

syntax_words = local_binder + [
    "Type",
    "Set",
    "Prop",
    "if",
    "then",
    "else",
    "match",
    "with",
    "end",
    "as",
    "in",
    "return",
    "using",
    "let"
]

vernacular_color = "#a020f0"
syntax_color = "#228b22"
global_bound_color = "#3b10ff"
local_bound_color = "#a0522d"
comment_color = "#004800"

def color_word(color : str, word : str) -> str:
    return "<span style=\"color:{}\">{}</span>".format(color, word)

def highlight_comments(page : str) -> str:
    result = ""
    comment_depth = 0
    for i in range(len(page)):
        if(page[i:i+2] == "(*"):
            comment_depth += 1
            if comment_depth == 1:
                result += "<span style=\"color:{}\">".format(comment_color)
        result += page[i]
        if(page[i-1:i+1] == "*)"):
            comment_depth -= 1
            if comment_depth == 0:
                result += "</span>"
    return result;

def syntax_highlight(page : str) -> str:
    for vernac in vernacular_words:
        page = re.sub(r"(?<!\")(?<!% )\b" + vernac + r"\b",
                      color_word(vernacular_color, vernac),
                      page)
    return highlight_comments(page);

def strip_comments(command : str) -> str:
    result = ""
    comment_depth = 0
    for i in range(len(command)):
        if command[i:i+2] == "(*":
            comment_depth += 1
        if comment_depth < 1:
            result += command[i]
        if command[i-1:i+1] == "*)":
            comment_depth -= 1
    return result
