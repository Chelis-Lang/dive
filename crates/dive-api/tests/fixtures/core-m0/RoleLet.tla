---- MODULE RoleLet ----
\* An action split inside a LET (DieHarder's JugToJug): its values read the
\* state and the split `\E` variables, and are lazy (`bad` fails if
\* evaluated, but never is).
EXTENDS Integers
VARIABLES c
J == {1, 2, 3}
Cap == [j \in J |-> j + 1]
Init == c = [j \in J |-> 0]
Fill(j) == c' = [c EXCEPT ![j] = Cap[j]]
Empty(j) == c' = [c EXCEPT ![j] = 0]
Pour(j, k) ==
  LET amt == IF c[j] < Cap[k] - c[k] THEN c[j] ELSE Cap[k] - c[k]
      bad == 1 \div 0
  IN  c' = [c EXCEPT ![j] = @ - amt, ![k] = @ + amt]
Next == \E j \in J : \/ Fill(j)
                     \/ Empty(j)
                     \/ \E k \in J \ {j} : Pour(j, k)
Inv == \A j \in J : c[j] \in 0..Cap[j]
====
