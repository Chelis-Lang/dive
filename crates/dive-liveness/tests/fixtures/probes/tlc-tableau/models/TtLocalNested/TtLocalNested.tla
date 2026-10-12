---- MODULE TtLocalNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Live == LET Identity(p) == p IN Identity([](x => []x))
====
