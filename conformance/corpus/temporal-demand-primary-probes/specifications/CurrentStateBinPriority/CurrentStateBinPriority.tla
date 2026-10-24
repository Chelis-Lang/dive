---- MODULE CurrentStateBinPriority ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == x' = 1 - x
TypeBad == Len(TRUE \o <<x>>) = 0
DivBad == IF x = 0 THEN 1 \div 0 = 0 ELSE TRUE
P == (<>[]TypeBad) \/ ([]<>DivBad)
====
