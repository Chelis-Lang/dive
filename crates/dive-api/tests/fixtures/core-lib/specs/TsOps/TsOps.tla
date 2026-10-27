---- MODULE TsOps ----
\* TLC!ToString in crates/dive-core/src/stdlib/tlc.rs, printed by
\* crates/dive-core/src/print.rs: TLC's single-line form. No interval or
\* lazy set occurs, so every enumerated set prints as TLC prints it (TsLazy
\* has those).
EXTENDS Naturals, Integers, Sequences, TLC
VARIABLE s
Init == s = ToString(0)
Next == \E k \in {1, 2, 3} : s' = ToString(<<k, {k}, [a |-> k]>>)
Inv == /\ s \in {"0"} \cup {ToString(<<k, {k}, [a |-> k]>>) : k \in {1, 2, 3}}
       /\ ToString(<<2, {2}, [a |-> 2]>>) = "<<2, {2}, [a |-> 2]>>"
       /\ ToString({3, 1, 2}) = "{1, 2, 3}"
       /\ ToString(<<1, "a">>) = "<<1, \"a\">>"
       /\ ToString(<<>>) = "<<>>"
       /\ ToString([a |-> 1, b |-> TRUE]) = "[a |-> 1, b |-> TRUE]"
       /\ ToString(2 :> 3) = "(2 :> 3)"
       /\ ToString((2 :> 3) @@ (4 :> 5)) = "(2 :> 3 @@ 4 :> 5)"
       /\ ToString(1 :> 3) = "<<3>>"
       /\ ToString("a" :> 3) = "[a |-> 3]"
       /\ ToString(-5) = "-5"
       /\ ToString(Nat) = "Nat"
       /\ ToString(Seq({1})) = "Seq({1})"
       /\ ToString({x \in {1, 2, 3, 4} : x > 2}) = "{3, 4}"
       /\ ToString(ToString(1)) = "\"1\""
       /\ ToString(2^20) = "1048576"
====
