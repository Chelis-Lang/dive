"""Writes parse-only cfg cases (TLC's ModelConfig is their oracle)."""
import os, sys

out = sys.argv[1]
os.makedirs(out, exist_ok=True)

CASES = {
    "private_keywords": "_PERIODIC P\n_RL_REWARD R\n_POSSIBLE A B\n_POSSIBLE C\nPOSTCONDITION Q\nPOSTCONDITIONS Q2\n",
    "bang_lhs": "CONSTANT A!B!C = 1\nX!Y <- Z\n",
    "module_forms": "CONSTANT X <- [M] Y\nZ = [Mod] 3\nW <- [M] V\nZ = [Mod] {a}\nX <- [M] Y2\n",
    "operator_args": "CONSTANT F(1, \"a\", {b}) = c\nG(TRUE) = FALSE\n",
    "nested_sets": "CONSTANT S = {{}, {a, {b}}, {1, 2}, {\"x\"}}\n",
    "typed_model_values": "CONSTANT S = {a_1, b_x, ab, _a}\n",
    "override_last_wins": "CONSTANT X <- A\nX <- B\nY <- A\n",
    "empty_section": "INVARIANT\nPROPERTY P\nCONSTANTS\nINIT I\n",
    "keyword_value_init": "INIT INIT\nNEXT N\n",
    "crlf": "INIT Init\r\nNEXT Next\r\nINVARIANT Inv\r\n",
    "cr_only_error": "INIT Init\rNEXT Next\rBOGUS\r",
    "crlf_error": "INIT Init\r\nNEXT Next\r\nBOGUS\r\n",
    "eof_line_newline": "INIT Init\nCONSTANT X =\n",
    "eof_line_no_newline": "INIT Init\nCONSTANT X =",
    "eof_line_cr": "INIT Init\nCONSTANT X =\r",
    "eof_line_crlf": "INIT Init\nCONSTANT X =\r\n",
    "eof_line_blank_lines": "INIT Init\nCONSTANT X =\n\n\n",
    "eof_line_trailing_comment": "INIT Init\nCONSTANT X = \\* nothing",
    "tabs": "INIT\tInit\n\tNEXT\t\tNext\n",
    "string_escape_quote": "CONSTANT S = \"a\\\"b\"\nT = \"\\\\\"\n",
    "string_bad_escape": "CONSTANT S = \"a\\qb\"\n",
    "numbers": "CONSTANT A = 007\nB = 0\nC = 2147483647\nD = 1a\n",
    "octal_number": "CONSTANT A = \\o17\n",
    "binary_number": "CONSTANT A = \\b101\n",
    "huge_number": "CONSTANT A = 99999999999999999999999\n",
    "unicode_ident": "CONSTANT S = {\u2115, \u2124}\n",
    "tla_keyword_names": "INVARIANT THEOREM ASSUME\nPROPERTY \"str\" 3\n",
    "same_line": "CONSTANT N = 1 CONSTANTS M = 2 INIT I NEXT N2\n",
    "constant_at_eof": "INIT I\nCONSTANT",
    "module_value_eof": "CONSTANT X = [M]",
    "module_value_open_eof": "CONSTANT X = [",
    "override_module_eof": "CONSTANT X <- [M]",
    "override_open_eof": "CONSTANT X <- [",
    "args_open_eof": "CONSTANT X(",
    "args_unclosed_eof": "CONSTANT X(1",
    "args_then_value": "CONSTANT X(1) 2\n",
    "args_ok": "CONSTANT X(1)= 2\n",
    "args_empty": "CONSTANT X() = 2\n",
    "deadlock_true": "CHECK_DEADLOCK TRUE\n",
    "deadlock_eof": "INIT I\nCHECK_DEADLOCK",
    "end_module_token": "INIT I\n====\n",
    "comment_in_value": "CONSTANT N = (* c *) 3\n",
    "comment_paren_star": "(*) INIT x *)\nNEXT N\n",
    "line_comment_cr": "\\* c\rINIT I\n",
    "set_eof": "CONSTANT S = {1, 2",
    "set_close_eof": "CONSTANT S = {",
    "set_comma_eof": "CONSTANT S = {1,",
    "lhs_bang_eof": "CONSTANT A!",
    "lhs_bang_bang": "CONSTANT A!!B = 1\n",
    "override_to_keyword": "CONSTANT X <- INIT\nNEXT N\n",
    "value_keyword": "CONSTANT X = INVARIANT\nINVARIANT I\n",
    "bad_char_question": "INIT I ?\n",
    "bad_char_backtick": "INIT `I`\n",
    "nul_byte": "INIT I\n\u0000\n",
    "form_feed": "INIT I\n\u000c\n",
    "nbsp": "INIT\u00a0I\n",
    "astral": "INIT I\n\U0001D538\n",
    "deep_set": "CONSTANT S = " + "{" * 1000 + "}" * 1000 + "\n",
    "args_section_line": "CONSTANT\nA = 1\nF(1 = 2\n",
    "twice_init_line": "INIT A\nNEXT B\n\nINIT\nC\n",
    "lex_error_after_parse_error": "INIT A B ;\n",
    "parse_error_after_lex_error": "INIT A ; B\n",
    "step_token": "INIT <1>2\n",
    "dash_separator": "INIT I\n----\n",
    "begin_module": "INIT I\n---- MODULE M ----\n",
    "unterminated_nested_comment": "INIT I\n(* a (* b *)\n",
    "comment_line_cr_eof": "INIT I \\* c\r",
}

for name, cfg in CASES.items():
    with open(os.path.join(out, name + ".cfg"), "w", encoding="utf-8", newline="") as f:
        f.write(cfg)
# Invalid UTF-8: a Latin-1 byte inside a comment, and outside one.
with open(os.path.join(out, "latin1_in_comment.cfg"), "wb") as f:
    f.write(b"\\* caf\xe9\nINIT I\n")
with open(os.path.join(out, "latin1_in_name.cfg"), "wb") as f:
    f.write(b"INIT caf\xe9\n")
print(len(CASES) + 2, "cases")
