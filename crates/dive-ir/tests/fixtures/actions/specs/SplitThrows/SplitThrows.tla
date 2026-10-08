---------------------------- MODULE SplitThrows ----------------------------
(* When splitting meets an operator that is not a definition (here the
   parameter b, bound to a value), TLC's cast to OpDefNode throws. The
   enclosing inlining catches it, keeps the actions already added (A) and
   adds the whole application as one action; an enclosing \E removes what
   its body added and becomes one action. An argument that fails to
   evaluate makes the application one action. *)
EXTENDS Naturals
VARIABLE x
A == x' = 1
F(b) == A \/ b
G(i) == x' = i \/ i
H(s) == x' = CHOOSE e \in s : TRUE
Next ==
  \/ F(x' = 2)
  \/ \E i \in {3, 4} : G(i = 3)
  \/ H(CHOOSE e \in {} : TRUE)
Init == x = 0
=============================================================================
