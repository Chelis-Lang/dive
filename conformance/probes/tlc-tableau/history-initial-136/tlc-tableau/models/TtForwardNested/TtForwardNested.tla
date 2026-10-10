---- MODULE TtForwardNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Identity(p) == p
Forward(q) == Identity(q)
Live == Forward([](x => []x))
====
