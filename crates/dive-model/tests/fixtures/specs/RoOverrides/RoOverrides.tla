---- MODULE RoOverrides ----
EXTENDS RoOverridesBase
VARIABLE x
Init == x = 0
Next == x' = x
Next2 == \E i \in Ids : x' = i
Inv == x \in Ids
Inv2 == x \in Ids \cup {9}
====
