---- MODULE TiTemplate ----
EXTENDS Integers
VARIABLE q
Prop == [](IF q=0 THEN <>(q=1) ELSE <>(q=2))
====
