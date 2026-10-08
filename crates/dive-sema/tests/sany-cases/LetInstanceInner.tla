---- MODULE LetInstanceInner ----
---- MODULE Inner ----
z == 1
====
F == LET I == INSTANCE Inner IN I!z
====
