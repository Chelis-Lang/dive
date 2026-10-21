---- MODULE Solutions ----
EXTENDS Naturals, FiniteSets
CONSTANT N
VARIABLE x
Init == x = {}
Toggle(i) == x' = IF i \in x THEN x \ {i} ELSE x \cup {i}
Next == \E i \in 1..N : Toggle(i)
Spec == Init /\ [][Next]_x /\ (\A i \in 1..N : SF_x(Toggle(i)))
Hold == \A i \in 1..N : []<>(i \in x)
Fail == \A i \in 1..N : <>[](i \in x)
FailOther == \A i \in 1..N : <>[](i \notin x)
Residual == []((1 \in x) => [](2 \in x))
====
