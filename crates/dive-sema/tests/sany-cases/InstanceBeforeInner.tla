---- MODULE InstanceBeforeInner ----
I == INSTANCE Inner2
---- MODULE Inner2 ----
z == 1
====
G == I!z
====
