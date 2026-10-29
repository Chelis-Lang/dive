---- MODULE GuardInner ----
EXTENDS Naturals
VARIABLE v
Double == IF v = v THEN 2*v ELSE 0
====
