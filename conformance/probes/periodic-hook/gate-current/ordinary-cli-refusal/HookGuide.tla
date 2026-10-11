---- MODULE HookGuide ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == UNCHANGED x
Live == []<>(x>=0)
Hook == FALSE
====
