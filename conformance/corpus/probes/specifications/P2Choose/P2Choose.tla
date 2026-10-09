---- MODULE P2Choose ----
\* Probe: which element does CHOOSE pick? The language only promises that
\* CHOOSE is deterministic; TLC picks the first satisfying element of the set
\* in its internal normalized order. For strings, TLC orders by interning
\* order of the string token, not lexicographically. Each invariant pins one
\* plausible answer; the cfg checks the "lexicographic" answers, so a
\* violation reveals which order the tool uses.
EXTENDS Integers
VARIABLES s, t, u
Init == /\ s = CHOOSE e \in {"zeta", "alpha", "mid"} : TRUE
        /\ t = CHOOSE e \in {{3}, {1, 2}, {}} : e # {}
        /\ u = CHOOSE e \in {<<2, 1>>, <<1, 2, 3>>, <<1, 3>>} : TRUE
Next == UNCHANGED <<s, t, u>>
SLex == s = "alpha"
TSetOrd == t = {3}
USeqOrd == u = <<1, 3>>
====
