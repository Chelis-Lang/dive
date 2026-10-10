---- MODULE TtEnabledNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Identity(p) == p
Live == Identity([](ENABLED Next => []x))
====
