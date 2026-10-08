---- MODULE ExtendsConflictKinds ----
---- MODULE A1 ----
x == 1
====
---- MODULE B1 ----
CONSTANT x
====
---- MODULE C1 ----
EXTENDS A1, B1
====
====
