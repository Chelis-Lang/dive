---- MODULE Resume ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=IF x<3 THEN x+1 ELSE x
Live == []<>(x>=0)
====
