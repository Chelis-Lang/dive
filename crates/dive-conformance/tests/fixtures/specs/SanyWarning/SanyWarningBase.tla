---- MODULE SanyWarningBase ----
EXTENDS Naturals
VARIABLE x
Foo == 1
Init == x = Foo
Next == UNCHANGED x
====
