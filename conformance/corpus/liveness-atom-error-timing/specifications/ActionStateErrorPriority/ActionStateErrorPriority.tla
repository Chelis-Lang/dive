---- MODULE ActionStateErrorPriority ----
EXTENDS Naturals, Sequences
VARIABLE x

Init == x = 0
Next == x' = 1 - x
StateBad == Len(TRUE \o <<x>>) = 0
ActionBad == IF x' = 1 THEN 1 \div 0 = 0 ELSE TRUE
P == ([]<>StateBad) /\ ([]<>ActionBad)
====
