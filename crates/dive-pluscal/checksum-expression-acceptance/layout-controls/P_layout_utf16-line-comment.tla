---- MODULE P_layout_utf16_line_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:await /\ TRUE \* 😀 comment
      /\ y=0; E:skip; end algorithm; *)
====
