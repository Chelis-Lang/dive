---- MODULE UnusedBadLevelInLet ----
VARIABLE x
F(a) == a'
G == LET H == F(x') IN TRUE
====
