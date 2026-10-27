---- MODULE TlcOps ----
\* The TLC module's value operators in crates/dive-core/src/stdlib/tlc.rs:
\* :>, @@ and Assert, in actions and invariants.
EXTENDS Naturals, TLC
VARIABLE f
Init == f = (1 :> 0)
Next == \E k \in 1..3 :
          /\ Assert(k \in 1..3, "k out of range")
          /\ f' = IF k \in DOMAIN f
                  THEN (k :> (f[k] + 1) % 3) @@ f
                  ELSE f @@ (k :> 0)
Inv == /\ DOMAIN (f @@ (5 :> 1)) = DOMAIN f \cup {5}
       /\ (f @@ (1 :> 7))[1] = f[1]
       /\ ((1 :> 7) @@ f)[1] = 7
       /\ ("a" :> 1) @@ ("b" :> 2) = [a |-> 1, b |-> 2]
       /\ <<1, 2>> @@ (3 :> 3) = <<1, 2, 3>>
       /\ (2 :> 2) @@ <<>> = (2 :> 2)
       /\ Assert(\A k \in DOMAIN f : f[k] < 3, f)
====
