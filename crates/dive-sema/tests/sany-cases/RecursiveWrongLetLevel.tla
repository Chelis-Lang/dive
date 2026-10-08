---- MODULE RecursiveWrongLetLevel ----
EXTENDS Naturals
RECURSIVE G(_)
F == LET G(m) == m IN 1
G(m) == m
====
