---- MODULE Inner ----
VARIABLE y
P(v) == v
Use(F(_)) == []<>F(y)
Live == Use(P)
====
